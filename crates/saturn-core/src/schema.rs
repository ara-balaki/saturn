//! The database's structure as the repository describes it, for the database diagram: tables, their
//! columns, and the foreign keys between them. Read from the schema files already in the project,
//! so no database connection is needed: a Prisma schema, or else SQL migrations applied in order.
//! Then laid out for drawing: related tables in columns from the most connected one outward.

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    Primary,
    Foreign,
    Unique,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Column {
    pub name: String,
    pub ty: String,
    pub key: Option<Key>,
    pub nullable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Table {
    pub name: String,
    pub columns: Vec<Column>,
}

/// A foreign key: `from_table.from_column` points at `to_table.to_column`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Relation {
    pub from_table: String,
    pub from_column: String,
    pub to_table: String,
    pub to_column: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Schema {
    pub tables: Vec<Table>,
    pub relations: Vec<Relation>,
    /// The files it was read from, relative to the repository.
    pub sources: Vec<PathBuf>,
}

/// Where the schema is described: Prisma schema files, else SQL migrations (in order), else a
/// `schema.sql`. Gitignored folders such as `node_modules` are skipped.
pub fn find_sources(repo: &Path) -> Vec<PathBuf> {
    let mut prisma = Vec::new();
    let mut sql = Vec::new();
    for entry in ignore::WalkBuilder::new(repo).max_depth(Some(8)).build().flatten() {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else { continue };
        let extension = path.extension().map(|ext| ext.to_string_lossy().to_lowercase()).unwrap_or_default();
        if extension == "prisma" {
            prisma.push(path.to_owned());
        } else if extension == "sql" {
            let in_migrations = path.components().any(|part| part.as_os_str().to_string_lossy().contains("migration"));
            if in_migrations || name == "schema.sql" {
                sql.push(path.to_owned());
            }
        }
    }
    let mut sources = if prisma.is_empty() { sql } else { prisma };
    sources.sort();
    sources
}

/// Reads the schema from `repo`'s schema files.
pub fn load(repo: &Path) -> Schema {
    let sources = find_sources(repo);
    let texts: Vec<String> = sources.iter().filter_map(|path| std::fs::read_to_string(path).ok()).collect();
    let prisma = sources.first().is_some_and(|path| path.extension().is_some_and(|ext| ext == "prisma"));
    let mut schema = if prisma { parse_prisma(&texts.join("\n")) } else { parse_sql(&texts) };
    schema.sources = sources.iter().map(|path| path.strip_prefix(repo).unwrap_or(path).to_owned()).collect();
    schema
}

/// Reads Prisma `model` blocks. A field whose type is another model is a relation, not a column;
/// the side with `@relation(fields: [...], references: [...])` holds the foreign key.
pub fn parse_prisma(text: &str) -> Schema {
    let blocks = prisma_models(text);
    let models: HashSet<&str> = blocks.iter().map(|(name, _)| name.as_str()).collect();
    let mut schema = Schema::default();
    for (name, body) in &blocks {
        let mut table = Table { name: name.clone(), columns: Vec::new() };
        let mut foreign: Vec<String> = Vec::new();
        for line in body.lines().map(|line| line.split("//").next().unwrap_or_default().trim()) {
            if line.is_empty() || line.starts_with("@@") {
                continue;
            }
            let mut words = line.split_whitespace();
            let (Some(field), Some(ty)) = (words.next(), words.next()) else { continue };
            let base = ty.trim_end_matches('?').trim_end_matches("[]");
            if models.contains(base) {
                if let Some((fields, references)) = relation_args(line) {
                    for (from, to) in fields.iter().zip(references.iter()) {
                        foreign.push(from.clone());
                        schema.relations.push(Relation {
                            from_table: name.clone(),
                            from_column: from.clone(),
                            to_table: base.to_owned(),
                            to_column: to.clone(),
                        });
                    }
                }
                continue;
            }
            let key = if line.contains("@id") {
                Some(Key::Primary)
            } else if line.contains("@unique") {
                Some(Key::Unique)
            } else {
                None
            };
            table.columns.push(Column {
                name: field.to_owned(),
                ty: base.to_owned(),
                key,
                nullable: ty.ends_with('?'),
            });
        }
        // A foreign key says more than "unique": a one-to-one relation's column is both.
        for column in &mut table.columns {
            if column.key != Some(Key::Primary) && foreign.contains(&column.name) {
                column.key = Some(Key::Foreign);
            }
        }
        schema.tables.push(table);
    }
    schema
}

/// `model Name { ... }` blocks, with their bodies.
fn prisma_models(text: &str) -> Vec<(String, String)> {
    let mut models = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find("model ") {
        let after = &rest[start + 6..];
        let Some(open) = after.find('{') else { break };
        let Some(close) = after[open..].find('}') else { break };
        let name = after[..open].trim();
        if !name.is_empty() && name.chars().all(|c| c.is_alphanumeric() || c == '_') {
            models.push((name.to_owned(), after[open + 1..open + close].to_owned()));
        }
        rest = &after[open + close..];
    }
    models
}

/// The `fields: [...]` and `references: [...]` of a Prisma `@relation(...)`.
fn relation_args(line: &str) -> Option<(Vec<String>, Vec<String>)> {
    let list = |key: &str| -> Option<Vec<String>> {
        let start = line.find(key)? + key.len();
        let open = line[start..].find('[')? + start + 1;
        let close = line[open..].find(']')? + open;
        Some(line[open..close].split(',').map(|item| item.trim().to_owned()).filter(|item| !item.is_empty()).collect())
    };
    Some((list("fields:")?, list("references:")?))
}

/// Applies SQL files in order: `CREATE TABLE` (with inline and table-level keys), `ALTER TABLE …
/// ADD COLUMN / ADD … FOREIGN KEY`, and `DROP TABLE`.
pub fn parse_sql(files: &[String]) -> Schema {
    let mut tables: Vec<Table> = Vec::new();
    let mut relations: Vec<Relation> = Vec::new();
    for statement in files.iter().flat_map(|file| split_statements(file)) {
        let lower = statement.to_lowercase();
        let words: Vec<&str> = lower.split_whitespace().collect();
        if words.starts_with(&["create", "table"]) {
            let Some(open) = statement.find('(') else { continue };
            let Some(close) = statement.rfind(')') else { continue };
            let name = table_name(&statement["create table".len()..open]);
            let mut table = Table { name: name.clone(), columns: Vec::new() };
            for item in split_top_level(&statement[open + 1..close]) {
                apply_definition(&mut table, &mut relations, &item);
            }
            tables.retain(|existing| existing.name != name);
            tables.push(table);
        } else if words.starts_with(&["alter", "table"]) {
            let body = statement["alter table".len()..].trim_start();
            let body = strip_prefix_ci(body, "if exists").unwrap_or(body).trim_start();
            let body = strip_prefix_ci(body, "only").unwrap_or(body).trim_start();
            let (name_part, action) = body.split_once(char::is_whitespace).unwrap_or((body, ""));
            let name = table_name(name_part);
            let Some(table) = tables.iter_mut().find(|table| table.name == name) else { continue };
            for clause in split_top_level(action) {
                let clause = clause.trim();
                if let Some(rest) = strip_prefix_ci(clause, "add") {
                    let rest = rest.trim_start();
                    let rest = strip_prefix_ci(rest, "column").unwrap_or(rest).trim_start();
                    apply_definition(table, &mut relations, rest);
                }
            }
        } else if words.starts_with(&["drop", "table"]) {
            let rest = statement["drop table".len()..].trim_start();
            let rest = strip_prefix_ci(rest, "if exists").unwrap_or(rest);
            let name = table_name(rest.trim().trim_end_matches(|c: char| c.is_whitespace() || c == ';'));
            tables.retain(|table| table.name != name);
            relations.retain(|relation| relation.from_table != name && relation.to_table != name);
        }
    }
    let names: HashSet<&str> = tables.iter().map(|table| table.name.as_str()).collect();
    relations
        .retain(|relation| names.contains(relation.from_table.as_str()) && names.contains(relation.to_table.as_str()));
    Schema { tables, relations, sources: Vec::new() }
}

/// One column or constraint inside `CREATE TABLE (...)` or after `ALTER TABLE … ADD`.
fn apply_definition(table: &mut Table, relations: &mut Vec<Relation>, item: &str) {
    let item = item.trim();
    let lower = item.to_lowercase();
    let first = lower.split_whitespace().next().unwrap_or_default();
    let constraint =
        matches!(first, "constraint" | "primary" | "foreign" | "unique" | "check" | "key" | "index" | "exclude");
    if constraint {
        let names = |after: &str| -> Vec<String> {
            lower.find(after).and_then(|at| paren_list(&item[at + after.len()..])).unwrap_or_default()
        };
        if lower.contains("primary key") {
            for name in names("primary key") {
                set_key(table, &name, Key::Primary);
            }
        }
        if lower.contains("foreign key") {
            let from = names("foreign key");
            if let Some(at) = lower.find("references") {
                let target = &item[at + "references".len()..];
                let table_end = target.find('(').unwrap_or(target.len());
                let to_table = table_name(&target[..table_end]);
                let to = paren_list(target).unwrap_or_default();
                for (index, column) in from.iter().enumerate() {
                    set_key(table, column, Key::Foreign);
                    relations.push(Relation {
                        from_table: table.name.clone(),
                        from_column: column.clone(),
                        to_table: to_table.clone(),
                        to_column: to.get(index).cloned().unwrap_or_else(|| "id".to_owned()),
                    });
                }
            }
        } else if first == "unique" || lower.starts_with("constraint") && lower.contains(" unique") {
            for name in names("unique") {
                set_key(table, &name, Key::Unique);
            }
        }
        return;
    }
    let mut parts = item.split_whitespace();
    let Some(name) = parts.next().map(unquote) else { return };
    let ty: String = parts
        .take_while(|word| {
            !matches!(
                word.to_lowercase().as_str(),
                "not" | "null" | "primary" | "references" | "unique" | "default" | "check" | "constraint" | "generated"
            )
        })
        .collect::<Vec<_>>()
        .join(" ");
    let key = if lower.contains("primary key") {
        Some(Key::Primary)
    } else if lower.contains("references") {
        Some(Key::Foreign)
    } else if lower.contains(" unique") {
        Some(Key::Unique)
    } else {
        None
    };
    let nullable = !lower.contains("not null") && key != Some(Key::Primary);
    if let Some(at) = lower.find("references") {
        let target = &item[at + "references".len()..];
        let table_end = target.find('(').unwrap_or(target.len());
        let to_table = table_name(target[..table_end].split_whitespace().next().unwrap_or_default());
        let to_column = paren_list(target).and_then(|list| list.into_iter().next()).unwrap_or_else(|| "id".to_owned());
        relations.push(Relation { from_table: table.name.clone(), from_column: name.clone(), to_table, to_column });
    }
    table.columns.retain(|column| column.name != name);
    table.columns.push(Column { name, ty: ty.to_lowercase(), key, nullable });
}

fn set_key(table: &mut Table, column: &str, key: Key) {
    if let Some(column) = table.columns.iter_mut().find(|candidate| candidate.name == column) {
        // Primary stays primary, and a foreign key isn't demoted to unique.
        let keep = column.key == Some(Key::Primary) || (column.key == Some(Key::Foreign) && key == Key::Unique);
        if !keep {
            column.key = Some(key);
        }
        if key == Key::Primary {
            column.nullable = false;
        }
    }
}

/// `"public"."orders"`, `` `orders` ``, `IF NOT EXISTS orders` → `orders`.
fn table_name(raw: &str) -> String {
    let raw = raw.trim();
    let raw = strip_prefix_ci(raw, "if not exists").unwrap_or(raw).trim();
    let last = raw.rsplit('.').next().unwrap_or(raw);
    unquote(last.trim())
}

fn unquote(name: &str) -> String {
    name.trim_matches(|c| matches!(c, '"' | '`' | '[' | ']')).to_owned()
}

fn strip_prefix_ci<'a>(text: &'a str, prefix: &str) -> Option<&'a str> {
    text.get(..prefix.len()).filter(|start| start.eq_ignore_ascii_case(prefix)).map(|_| &text[prefix.len()..])
}

/// The names in the first `( … )` of `text`.
fn paren_list(text: &str) -> Option<Vec<String>> {
    let open = text.find('(')? + 1;
    let close = text[open..].find(')')? + open;
    Some(text[open..close].split(',').map(|name| unquote(name.trim())).filter(|name| !name.is_empty()).collect())
}

/// SQL statements without comments, split on `;`.
fn split_statements(sql: &str) -> Vec<String> {
    let mut clean = String::with_capacity(sql.len());
    let mut rest = sql;
    while !rest.is_empty() {
        if let Some(after) = rest.strip_prefix("--") {
            rest = after.split_once('\n').map_or("", |(_, next)| next);
            clean.push('\n');
        } else if let Some(after) = rest.strip_prefix("/*") {
            rest = after.split_once("*/").map_or("", |(_, next)| next);
        } else {
            let next = rest.chars().next().unwrap_or_default();
            clean.push(next);
            rest = &rest[next.len_utf8()..];
        }
    }
    clean.split(';').map(|statement| statement.trim().to_owned()).filter(|statement| !statement.is_empty()).collect()
}

/// Splits on commas that aren't inside parentheses.
fn split_top_level(text: &str) -> Vec<String> {
    let (mut parts, mut depth, mut current) = (Vec::new(), 0_i32, String::new());
    for c in text.chars() {
        match c {
            '(' => depth += 1,
            ')' => depth -= 1,
            ',' if depth == 0 => {
                parts.push(std::mem::take(&mut current));
                continue;
            }
            _ => {}
        }
        current.push(c);
    }
    if !current.trim().is_empty() {
        parts.push(current);
    }
    parts
}

/// Where a table's box goes, in diagram units (pixels at 100%).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Placed {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

pub const BOX_WIDTH: f32 = 240.;
pub const HEADER_HEIGHT: f32 = 34.;
pub const ROW_HEIGHT: f32 = 24.;
const COLUMN_GAP: f32 = 96.;
const ROW_GAP: f32 = 32.;

/// A box's height for `columns` columns.
pub fn box_height(columns: usize) -> f32 {
    HEADER_HEIGHT + ROW_HEIGHT * f32::from(u16::try_from(columns).unwrap_or(u16::MAX)) + 8.
}

/// Places every table. Each group of related tables starts from its most connected table and
/// spreads right, one column per step along its relations; groups stack downwards, and tables
/// with no relations end up in a grid at the bottom.
pub fn layout(schema: &Schema) -> Vec<Placed> {
    let index: HashMap<&str, usize> =
        schema.tables.iter().enumerate().map(|(i, table)| (table.name.as_str(), i)).collect();
    let mut neighbours: Vec<Vec<usize>> = vec![Vec::new(); schema.tables.len()];
    for relation in &schema.relations {
        if let (Some(&a), Some(&b)) = (index.get(relation.from_table.as_str()), index.get(relation.to_table.as_str()))
            && a != b
        {
            neighbours[a].push(b);
            neighbours[b].push(a);
        }
    }
    let mut placed = vec![Placed { x: 0., y: 0., width: BOX_WIDTH, height: 0. }; schema.tables.len()];
    for (i, table) in schema.tables.iter().enumerate() {
        placed[i].height = box_height(table.columns.len());
    }
    let mut seen = vec![false; schema.tables.len()];
    let mut order: Vec<usize> = (0..schema.tables.len()).collect();
    order.sort_by_key(|&i| std::cmp::Reverse(neighbours[i].len()));
    let mut top = 0.0_f32;
    let mut loners = Vec::new();
    for &start in &order {
        if seen[start] {
            continue;
        }
        if neighbours[start].is_empty() {
            seen[start] = true;
            loners.push(start);
            continue;
        }
        // Breadth first from the hub: each step along a relation is one column further right.
        let mut layers: Vec<Vec<usize>> = Vec::new();
        let mut queue = VecDeque::from([(start, 0_usize)]);
        seen[start] = true;
        while let Some((table, depth)) = queue.pop_front() {
            if layers.len() <= depth {
                layers.push(Vec::new());
            }
            layers[depth].push(table);
            for &next in &neighbours[table] {
                if !seen[next] {
                    seen[next] = true;
                    queue.push_back((next, depth + 1));
                }
            }
        }
        let mut bottom = top;
        for (depth, layer) in layers.iter().enumerate() {
            let x = f32::from(u16::try_from(depth).unwrap_or(u16::MAX)) * (BOX_WIDTH + COLUMN_GAP);
            let mut y = top;
            for &table in layer {
                placed[table].x = x;
                placed[table].y = y;
                y += placed[table].height + ROW_GAP;
            }
            bottom = bottom.max(y);
        }
        top = bottom + ROW_GAP;
    }
    // Unrelated tables: a grid, four across.
    let mut row_height = 0.0_f32;
    for (n, &table) in loners.iter().enumerate() {
        let column = n % 4;
        if column == 0 && n > 0 {
            top += row_height + ROW_GAP;
            row_height = 0.;
        }
        placed[table].x = f32::from(u16::try_from(column).unwrap_or(0)) * (BOX_WIDTH + COLUMN_GAP);
        placed[table].y = top;
        row_height = row_height.max(placed[table].height);
    }
    placed
}

/// The zoom (at most 1) and offset that fit a `width` × `height` diagram into a `view_width` ×
/// `view_height` view with `margin` around it, centred when it's smaller than the view.
pub fn fit(width: f32, height: f32, view_width: f32, view_height: f32, margin: f32, min_zoom: f32) -> (f32, f32, f32) {
    let (room_w, room_h) = ((view_width - margin * 2.).max(1.), (view_height - margin * 2.).max(1.));
    if width <= 0. || height <= 0. {
        return (1., margin, margin);
    }
    let zoom = (room_w / width).min(room_h / height).clamp(min_zoom, 1.);
    (zoom, margin + (room_w - width * zoom).max(0.) / 2., margin + (room_h - height * zoom).max(0.) / 2.)
}

#[cfg(test)]
mod tests {
    use super::*;

    const PRISMA: &str = r#"
        datasource db { provider = "postgresql" url = env("DATABASE_URL") }
        model User {
          id      Int      @id @default(autoincrement())
          email   String   @unique
          name    String?
          posts   Post[]
        }
        // A blog post.
        model Post {
          id       Int    @id
          title    String
          author   User   @relation(fields: [authorId], references: [id])
          authorId Int
          @@index([authorId])
        }
        enum Role { ADMIN USER }
    "#;

    #[test]
    fn reads_prisma_models_columns_and_relations() {
        let schema = parse_prisma(PRISMA);
        assert_eq!(schema.tables.iter().map(|t| t.name.as_str()).collect::<Vec<_>>(), ["User", "Post"]);
        let user = &schema.tables[0];
        assert_eq!(user.columns.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(), ["id", "email", "name"]);
        assert_eq!(user.columns[0].key, Some(Key::Primary));
        assert_eq!(user.columns[1].key, Some(Key::Unique));
        assert!(user.columns[2].nullable);
        let post = &schema.tables[1];
        assert_eq!(post.columns.iter().find(|c| c.name == "authorId").and_then(|c| c.key), Some(Key::Foreign));
        assert_eq!(
            schema.relations,
            [Relation {
                from_table: "Post".into(),
                from_column: "authorId".into(),
                to_table: "User".into(),
                to_column: "id".into()
            }]
        );
    }

    #[test]
    fn applies_sql_migrations_in_order() {
        let first = r#"
            -- The first migration.
            CREATE TABLE IF NOT EXISTS "public"."users" (
                id SERIAL PRIMARY KEY,
                email VARCHAR(255) NOT NULL UNIQUE,
                created_at TIMESTAMP DEFAULT now()
            );
            CREATE TABLE orders (
                id BIGINT NOT NULL,
                user_id INT NOT NULL REFERENCES users(id),
                total NUMERIC(10, 2),
                PRIMARY KEY (id)
            );
            CREATE TABLE scratch (id INT);
        "#;
        let second = r"
            ALTER TABLE orders ADD COLUMN coupon_id INT;
            CREATE TABLE coupons (id INT PRIMARY KEY, code TEXT);
            ALTER TABLE orders ADD CONSTRAINT orders_coupon FOREIGN KEY (coupon_id) REFERENCES coupons (id);
            /* gone */ DROP TABLE IF EXISTS scratch;
        ";
        let schema = parse_sql(&[first.to_owned(), second.to_owned()]);
        assert_eq!(schema.tables.iter().map(|t| t.name.as_str()).collect::<Vec<_>>(), ["users", "orders", "coupons"]);
        let users = &schema.tables[0];
        assert_eq!(users.columns[0].key, Some(Key::Primary));
        assert_eq!(users.columns[1].ty, "varchar(255)");
        assert_eq!(users.columns[1].key, Some(Key::Unique));
        assert!(!users.columns[1].nullable);
        let orders = &schema.tables[1];
        assert_eq!(
            orders.columns.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(),
            ["id", "user_id", "total", "coupon_id"]
        );
        assert_eq!(orders.columns[0].key, Some(Key::Primary));
        assert_eq!(orders.columns[2].ty, "numeric(10, 2)");
        assert_eq!(orders.columns[3].key, Some(Key::Foreign));
        assert_eq!(
            schema.relations.iter().map(|r| (r.from_column.as_str(), r.to_table.as_str())).collect::<Vec<_>>(),
            [("user_id", "users"), ("coupon_id", "coupons")]
        );
    }

    #[test]
    fn lays_related_tables_out_from_the_hub() {
        let schema = parse_prisma(PRISMA);
        let placed = layout(&schema);
        assert_eq!(placed.len(), 2);
        // User and Post are related: side by side, one column apart.
        assert!((placed[1].x - placed[0].x).abs() > BOX_WIDTH);
        assert!((placed[0].height - box_height(3)).abs() < f32::EPSILON);
    }

    #[test]
    fn fits_the_diagram_into_the_view() {
        // Twice as wide as the room: half size, centred vertically.
        let (zoom, x, y) = fit(2_000., 400., 1_032., 632., 16., 0.1);
        assert!((zoom - 0.5).abs() < f32::EPSILON);
        assert!((x - 16.).abs() < f32::EPSILON && (y - (16. + 200.)).abs() < f32::EPSILON);
        // Smaller than the room: never zoomed past 100%, centred both ways.
        let (zoom, x, _) = fit(400., 100., 1_032., 632., 16., 0.1);
        assert!((zoom - 1.).abs() < f32::EPSILON && (x - 316.).abs() < f32::EPSILON);
        // Huge: no smaller than the minimum.
        assert!((fit(100_000., 100_000., 500., 500., 16., 0.15).0 - 0.15).abs() < f32::EPSILON);
    }

    #[test]
    fn finds_prisma_first_then_migrations() {
        let dir = tempfile::tempdir().expect("a temp dir");
        std::fs::create_dir_all(dir.path().join("db/migrations")).expect("folders");
        std::fs::write(dir.path().join("db/migrations/002_more.sql"), "CREATE TABLE b (id INT);").expect("written");
        std::fs::write(dir.path().join("db/migrations/001_init.sql"), "CREATE TABLE a (id INT);").expect("written");
        std::fs::write(dir.path().join("notes.sql"), "SELECT 1;").expect("written");
        let schema = load(dir.path());
        assert_eq!(schema.tables.iter().map(|t| t.name.as_str()).collect::<Vec<_>>(), ["a", "b"]);
        assert_eq!(schema.sources.len(), 2);
        std::fs::create_dir_all(dir.path().join("prisma")).expect("folder");
        std::fs::write(dir.path().join("prisma/schema.prisma"), PRISMA).expect("written");
        assert_eq!(load(dir.path()).tables.len(), 2);
        assert_eq!(load(dir.path()).sources, [PathBuf::from("prisma/schema.prisma")]);
    }
}
