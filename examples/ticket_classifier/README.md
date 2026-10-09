# Selected-ticket classification port

Source: the sibling Go clasificador-tickets project's Chatwoot client,
app.buildTicket and classifier.Classify. The Go checkout is unchanged.

```sh
tarn build examples/ticket_classifier/main.tarn -o /tmp/ticket-classifier
# Configure CHATWOOT_TOKEN and TYPESAFE_API_KEY securely in the environment.
/tmp/ticket-classifier https://your-chatwoot-host 1 7 categories.json
```

The base URL must have no trailing slash. categories.json contains the Choice
criteria object, for example `{"otro":"Other support requests"}`; it is not the
whole Go configuration file. Credentials are never arguments, logs or output.
TYPESAFE_ENDPOINT optionally overrides the default evaluation endpoint;
TARN_CA_FILE optionally supplies a trusted PEM CA file. Empty CA uses system trust.
The system must provide libcurl.so.4. No curl executable or Cargo SDK is needed.

This CLI fetches a selected conversation and its messages, skips private/system/
blank messages, sorts by creation time, keeps the last 20, truncates each to
2,000 Unicode scalars, rejects tickets without customer messages, and asks Jev
for category, urgency and customer frustration in a single request. It writes
the complete JSON response, preserving numeric spelling instead of converting
scores to integers. Confidence/priority decisions are not applied automatically.

All requests have a 30-second libcurl timeout and a 1 MiB response limit. They
block the current task. Certificate and hostname validation are always enabled;
redirects, proxies and retries are disabled. Failure exits 1 and prints a code
without credentials or response content: -3 input/config, -4 JSON, -5 output,
-6 no customer text; -1000 minus HTTP status; otherwise https.Error's native code.

The deterministic acceptance test uses mock Chatwoot and TypeSafe HTTPS servers
with fixture credentials. No live ticket or external AI request is made by tests.
Run `cargo test -p tarn_backend --test text_https` on a host permitting local sockets
and providing Python 3, OpenSSL and libcurl.

This is a console workflow port, not the complete web application: UI/auth,
pagination, polling, worker pools, cached state, label/priority writes, automatic
apply and 429/529 retry/Retry-After are not ported. Equal timestamp ordering is
unstable; absent private flags are conservatively treated as private. Categories
are provided by the operator and model behavior needs separate evaluation on real
data before any operational use.
