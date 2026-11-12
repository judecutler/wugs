# wugs help bot

The **Help** button in the app asks this Cloudflare Worker. It reads the user guide (`docs/USER-GUIDE.md`, at the tag of the asker's version, else `main`), picks the sections that match the question, and asks a small hosted model to answer from those sections only. The answer comes back with the sections it used, which the app links.

- **Model.** Workers AI, `@cf/meta/llama-3.1-8b-instruct-fp8` by default (`MODEL` in `wrangler.toml`; any Workers AI text model works). A question costs about 20 neurons, so the free 10,000 a day cover about 500 questions; past that a free account stops answering until the next day and a paid one is billed $0.011 per 1,000 neurons. To answer through DeepSeek instead, set `PROVIDER = "deepseek"` in `[vars]` and `npx wrangler@4 secret put DEEPSEEK_API_KEY`.
- **Nothing kept unless agreed.** The app sends a question with `share: true` only when the person ticked the box. Shared questions (the question, the guide section it matched, the app version; no project data, no address) go to the Analytics Engine dataset `wugs_help_asked`, which keeps them three months. Unshared questions are not stored; Cloudflare's request logs (`[observability]`) keep request metadata, not bodies.
- **Guide changes need no deploy.** The guide is fetched from GitHub and cached for an hour.
- **Limits.** 10 questions a minute per address. Questions up to 1,000 characters; the app sends at most the last 6 turns.

## Deploy

`.github/workflows/help.yml` deploys on every push to `main` that touches this folder, and on demand (Actions → help → Run workflow). It needs two repository secrets: `CLOUDFLARE_ACCOUNT_ID`, and `CLOUDFLARE_API_KEY` holding an **API token** (not the Global API Key) with *Workers Scripts: Edit*, *Workers AI: Read* and *Account Analytics: Read*. The run's summary shows the service's address. Put it in `ENDPOINT` in `apps/desktop/src-tauri/src/help.rs`; builds made after that answer questions, and until then the Help sheet says it is not connected yet.

By hand, from this folder: `npx wrangler@4 login`, then `npx wrangler@4 deploy`.

## The "most asked" report

Actions → help → Run workflow with **report** ticked prints the guide sections people asked about most in the last 30 days, with recent questions under each, in the run's summary. Locally: `CLOUDFLARE_ACCOUNT_ID=… CLOUDFLARE_API_TOKEN=… npm run most-asked -- 30`.

## Run your own

Anyone can: fork, point `GUIDE_URL` at your guide, deploy with your own Cloudflare account, and set `ENDPOINT` in your build.

## Test

`npm test` runs the search against the real guide and the Worker with the model stubbed. CI runs it.
