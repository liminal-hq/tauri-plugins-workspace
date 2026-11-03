---
'steamworks': minor
'steamworks-js': minor
---

Adds Steam Cloud file commands to the `steamworks` plugin: `cloudWrite`, `cloudRead`, `cloudReadText`, `cloudDelete`, `cloudList`, `cloudQuota` and `cloudStatus`. File bytes travel as the raw IPC body rather than JSON, names are limited to safe relative paths, and the whole group sits behind the opt-in `allow-cloud` permission set. The README points games at Auto-Cloud first, which needs no code.
