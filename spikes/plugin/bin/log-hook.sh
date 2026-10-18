#!/bin/sh
# Appends each hook's input to $SATURN_SPIKE_HOOK_LOG. On a prompt that starts with a slash
# command whose first argument is a ticket, answers with a sessionTitle, as Saturn will.
input=$(cat)
[ -n "$SATURN_SPIKE_HOOK_LOG" ] && printf '%s\n' "$input" >> "$SATURN_SPIKE_HOOK_LOG"
event=$(printf '%s' "$input" | jq -r '.hook_event_name // empty')
if [ "$event" = "UserPromptSubmit" ]; then
  ticket=$(printf '%s' "$input" | jq -r '.prompt' | sed -nE 's#^/[a-zA-Z0-9:_-]+ +([A-Za-z]+-[0-9]+).*#\1#p' | tr '[:lower:]' '[:upper:]')
  if [ -n "$ticket" ]; then
    jq -nc --arg title "$ticket" '{hookSpecificOutput: {hookEventName: "UserPromptSubmit", sessionTitle: $title}}'
  fi
fi
exit 0
