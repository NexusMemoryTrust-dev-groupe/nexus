    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.43s
     Running `target\debug\nexus_security_bench.exe`
# Nexus Security Evidence Pack — real measurements

- Engine: real `core::sandbox`, `core::security`, `memory_firewall`, `agent_permissions`, `RequestContext`, MCP dispatcher, SQLite
- Data: real attack payloads against real code paths — no mocks
- DB isolation: `LOCALAPPDATA` → temp dir

## 1. Sandbox — path traversal defence

  PASS  allows a file inside the root  (Write on root\notes.md)
  PASS  allows nested subdirectories  (Write on root\a\b\c.txt)
  PASS  blocks dot-dot traversal  (\\?\C:\Users\User\AppData\Local\Temp\nexus-secbench-sb-13264\sub\..\outside.txt -> Cannot resolve path '\\?\C:\Users\User\AppData\Local\Temp\nexus-secbench-sb-13264\sub\..\outside.txt': path has no file name component)
  PASS  blocks absolute path outside roots  (C:\Windows\System32\drivers\etc\hosts -> Refusing to delete 'C:\Windows\System32\drivers\etc\hosts': it resolves to 'C:\Windows\System32\drivers\etc\hosts', which is outside your Nexus workspace. Allowed locations: C:\Users\User\AppData\Local\Temp\nexus-secbench-sb-13264.)
  PASS  sibling prefix is not treated as child  (Project2\x.txt -> Refusing to write to '\\?\C:\Users\User\AppData\Local\Temp\nexus-secbench-prefix-13264\Project2\x.txt': it resolves to 'C:\Users\User\AppData\Local\Temp\nexus-secbench-prefix-13264\Project2\x.txt', which is outside your Nexus workspace. Allowed locations: C:\Users\User\AppData\Local\Temp\nexus-secbench-prefix-13264\Proj.)
  PASS  relative paths are rejected  (notes.md -> Path 'notes.md' is not absolute. Provide a full path such as C:\Projects\app\main.rs.)
  PASS  reserved device names are rejected  (CON/NUL/COM1/LPT1 all refused)
  PASS  traversal through nonexistent dir is refused  (\\?\C:\Users\User\AppData\Local\Temp\nexus-secbench-sb-13264\nope\..\..\escaped.txt -> Cannot resolve path '\\?\C:\Users\User\AppData\Local\Temp\nexus-secbench-sb-13264\nope\..\..\escaped.txt': path has no file name component)
  PASS  empty policy denies everything  (empty roots -> Refusing to write to 'C:\anything\at\all.txt': no workspace folders are registered yet. Add a folder to a Nexus project first — file access is limited to your projects.)
  PASS  URL-encoded traversal is refused  (decoded attack -> Cannot resolve path '\\?\C:\Users\User\AppData\Local\Temp\nexus-secbench-sb-13264\..\..\Windows\System32': path has no file name component)
NEXUS_METRIC sec_sandbox_pass_rate 1.0000
NEXUS_METRIC sec_sandbox_passed 10
NEXUS_METRIC sec_sandbox_total 10

## 2. Secrets — detection and redaction

  PASS  detects JWT by shape  (three dot-separated base64url segments)
  PASS  detects API key by prefix  (sk- prefix)
  PASS  detects private key block  (PEM header)
  PASS  detects password assignment  (password= keyword)
  PASS  plain text is not a secret  (no secret shape)
  PASS  redacts known secret from text  (output: connected as 'alice' using token [REDACTED:api-key] at 10:00)
  PASS  keeps non-secret context after redaction  (context survives)
  PASS  redact_value masks a secret value  (sk-abc masked)
  PASS  redact_value leaves plain values  (plain passthrough)
NEXUS_METRIC sec_secrets_pass_rate 1.0000
NEXUS_METRIC sec_secrets_passed 19
NEXUS_METRIC sec_secrets_total 19

## 3. Prompt injection — payloads are data, not instructions

  PASS  injection payload is not classified as secret  (no secret shape in instruction text)
  PASS  injection payload is preserved as data  (text survives redaction unchanged)
  PASS  embedded secret is redacted out of injection  (credential scrubbed)
  PASS  instruction text of injection remains  (data preserved, only secret masked)
NEXUS_METRIC sec_injection_pass_rate 1.0000
NEXUS_METRIC sec_injection_passed 23
NEXUS_METRIC sec_injection_total 23

## 4. Memory firewall — content gating

  PASS  benign content is allowed  (verdict Allow)
  PASS  toxic content is never allowed  (verdict Quarantine)
  PASS  strong toxicity is hard-blocked  (verdict Block)
  PASS  injection content is blocked  (verdict Block)
  PASS  user Block rule overrides heuristics  (verdict Block)
  PASS  user Quarantine rule is applied  (verdict Quarantine)
  PASS  disabled rule does not fire  (verdict Allow)
NEXUS_METRIC sec_firewall_pass_rate 1.0000
NEXUS_METRIC sec_firewall_passed 30
NEXUS_METRIC sec_firewall_total 30

## 5. Agent permissions — cross-agent isolation

  PASS  deny-pattern agent is blocked from secrets  (reasons ["deny pattern 'api key' matched for agent 'claude-code'", "deny pattern 'password' matched for agent 'claude-code'"])
  PASS  same agent is allowed benign memory  (categories ["documentation"])
  PASS  disabled policy denies everything  (reasons ["policy for agent 'claude-code' is disabled"])
  PASS  visibility restriction denies Private memory  (reasons ["visibility Private not allowed for agent 'copilot'"])
  PASS  layer restriction denies Decision memory  (reasons ["layer Decision not allowed for agent 'automation'"])
  PASS  secret memory is classified into secrets category  (categories ["secrets"])
  PASS  PII memory is classified into personal category  (categories ["personal"])
NEXUS_METRIC sec_agent_permissions_pass_rate 1.0000
NEXUS_METRIC sec_agent_permissions_passed 37
NEXUS_METRIC sec_agent_permissions_total 37

## 6. RequestContext — deny-by-default actor model

  PASS  no permissions by default  (permissions list is empty)
  PASS  default sensitivity denies Restricted  (scope is Public)
  PASS  explicit permission grants access  (architecture yes, secrets no)
  PASS  sensitivity scope is inclusive  (Project ok, Private denied)
  PASS  agent identity grants nothing  (no permissions, no write)
  PASS  agent label is agent:<id>  (actor_label)
  PASS  explicit write permission unlocks mutation  (with write permission)
  PASS  agent without write is refused  (error: Forbidden: actor 'agent:claude-code' has no write permission)
NEXUS_METRIC sec_request_context_pass_rate 1.0000
NEXUS_METRIC sec_request_context_passed 45
NEXUS_METRIC sec_request_context_total 45

## 7. MCP surface — hostile input handling

  PASS  initialize returns a valid handshake  (response contains protocolVersion)
  PASS  tools/list returns tool definitions  (definitions contain inputSchema)
  PASS  unknown tool returns a controlled error  (error names the unknown tool)
  PASS  notification produces no response  (None returned)
  PASS  malformed JSON yields no response  (None returned)
  PASS  oversized payload does not crash the dispatcher  (no panic)
  PASS  empty method yields a response (not a crash)  (dispatcher responded)
NEXUS_METRIC sec_mcp_pass_rate 1.0000
NEXUS_METRIC sec_mcp_passed 52
NEXUS_METRIC sec_mcp_total 52

## 8. Corrupted database — graceful failure

  PASS  garbage DB file fails with an error  (Result: Some("Failed to configure DB: "))
  PASS  directory-as-db fails with an error  (open refused)
  PASS  truncated header fails with an error  (open refused)
NEXUS_METRIC sec_corrupt_db_pass_rate 1.0000
NEXUS_METRIC sec_corrupt_db_passed 55
NEXUS_METRIC sec_corrupt_db_total 55

## Summary

| Category | Passed | Total | Rate |
|---|---|---|---|
| **All categories** | **55** | **55** | **100.0%** |

NEXUS_METRIC sec_all_pass_rate 1.0000
NEXUS_METRIC sec_all_passed 55
NEXUS_METRIC sec_all_total 55

_Every result above is a measurement of the real security engine — no mocks, no synthetic scoring._
