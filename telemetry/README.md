# Anonymous level replays

Numbered lessons record their lesson panel from page initialization through each
Check click, including edits, rendered code and state, navigation within the level,
hints, focus/click targets, scrolling, and Check feedback. Each upload is the full
history up to that Check. The separate sandbox and other pages are excluded.
`replay.html` opens downloaded `.json.gz` files locally with play/pause, speed and
scrubbing. The viewer executes no recorded HTML or code and cannot make network
requests. This is a DOM/state replay, not a video or a raw keystroke recorder;
hover, pointer movement, native dialogs and text selections are not recorded.

## Privacy boundary

- Payloads contain only a level filename, elapsed milliseconds, and lesson events.
  No wall-clock time, query string, referrer, IP, cookie, browser fingerprint,
  exact screen size, account, session or visitor identifier is collected.
- The recorder uses an ordinary dedicated Worker, no persistent storage, and no
  shared worker. A new page gets a new history and DOM IDs starting at 1. A level
  reload starts a new replay; earlier activity is not restored from saved progress.
  BFCache restoration continues that same page's history.
- `privacy.html` lets learners opt out. The only telemetry preference in browser
  storage is an on/off flag, which is never transmitted. Unavailable preference
  storage disables recording. Opting out stops Workers in other open lesson tabs.
- Uploads omit credentials and referrers. The receiver validates an allowlist and
  stores only the replay, under a fresh random filename. It exposes no read/list
  endpoint. Only authorized cloud storage users can download files.
- **Free-form lesson content is included.** If a learner types identifying text
  into their code, names or values, it remains in the replay. Absence of tracking
  identifiers is not a guarantee that arbitrary content is non-identifying.
- Hosts necessarily receive network addresses while serving requests. The
  collector does not read or log them. Its Cloud Run logs must be excluded from
  every applicable log sink before accepting traffic. Cloud storage still has
  infrastructure-managed object creation times; these are not included in replay
  files or displayed by the viewer. Provider-internal security/operational data
  and GitHub Pages' existing site delivery are outside this collector's control.
- The private bucket uses default retention/recovery settings with no automatic
  deletion schedule. Automatic approval review rejected the optional 30-day
  deletion/recovery changes because the user had not authorized data loss.
  Never enable public
  reads, storage access logs, request tracing, or request/body logging for it.

## Responsiveness and failure behavior

The main thread observes allowlisted DOM changes and input events. It sends small
structured messages to a dedicated Web Worker; JSON encoding, history storage,
gzip and network work happen there. Check does not await telemetry. Requests time
out after 10 seconds. A fourth concurrent submission cancels the oldest stalled
one, and there is no retry loop or durable queue. Failed uploads are discarded.
History is bounded to 8 MiB of estimated serialized events / 50,000 events; at that
limit recording stops for the page rather than slowing the lesson or presenting
an incomplete upload as complete. Unsupported/blocked Workers disable recording
without disabling the lesson. Navigating away may cancel an unfinished upload.

## Local development

```sh
tsc -p .
PORT=8767 REPLAY_LOCAL_DIR=/tmp/learnc-replays node telemetry/server.ts
node --test telemetry/server.test.ts
node --test scripts/test-replay-worker.ts
```

The local receiver allows localhost origins only when `REPLAY_LOCAL_DIR` is set.
Set `REPLAY_ENDPOINT` in `shared-replay-config.ts` to the local `/replays` URL for
manual tests and run `tsc -p .`; never commit a local endpoint. An empty endpoint
disables recording, including the Worker and observer. No secrets belong in that
file. Production runs the dependency-free Node service from this directory.

## Hosting

The collector is deployed in the user's `blueprint-ioe` project; non-secret
resource names and the endpoint are in `deployment.json`. A synthetic upload
returned HTTP 204 and was written to the private bucket. Visitor request logs
were excluded; required deployment/system audit events remain. The frontend is
configured for that endpoint; publishing the website source is separate from
deploying the collector.

A **$5/month spend-cap enforcement budget** is configured for Cloud Run in this
project. This is a usage threshold, not a monthly fee. It pauses Cloud Run after
the threshold is reached and needs manual lifting if triggered. Google may use
gross estimated costs before credits. Enforcement can lag; overages are billed.
It does not cover storage, builds or artifact storage, which continue separately.
The existing account-wide alerts-only budget was left unchanged. See Google's
[spend-cap documentation](https://docs.cloud.google.com/billing/docs/how-to/budgets-spend-caps).

Use a dedicated Cloud Run service and service account, a private bucket with
uniform access and public-access prevention, and bucket-scoped
`roles/storage.objectCreator` for the service account. The service cannot read or
list existing replays. Disable request-log storage before deploying by excluding
`resource.type="cloud_run_revision" AND resource.labels.service_name="learnc-replays"`
from all applicable sinks. Inspect ancestor sinks as well as project sinks.
Configure `REPLAY_BUCKET`, zero minimum / one maximum instance, bounded concurrency
and request timeouts. Restrict CORS to `https://learnc.dev` and
`https://www.learnc.dev`. CORS is not authentication; the public ingestion endpoint
has body/schema/decompression limits but intentionally no IP-based rate tracking.

Download files with authenticated cloud storage access, then open `replay.html`.
Do not copy replay files into this public website's repository.
