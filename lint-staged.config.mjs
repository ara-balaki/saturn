// Formats staged files before each commit. The vendored GPUI in vendor/ keeps upstream's
// formatting, so a later GPUI can be copied in and the patch re-applied cleanly.
const own = (files) => files.filter((file) => !file.includes('/vendor/'));
const run = (command) => (files) => {
  const kept = own(files);
  return kept.length ? [`${command} ${kept.map((file) => JSON.stringify(file)).join(' ')}`] : [];
};

export default {
  '*.rs': run('rustfmt --edition 2024'),
  '*.{json,md,yml,yaml,toml}': run('oxfmt --no-error-on-unmatched-pattern'),
};
