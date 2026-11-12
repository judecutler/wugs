# wugs help bot

The app's **Help** panel (the wug answers in speech bubbles) asks this Cloudflare Worker. It reads the user guide (`docs/USER-GUIDE.md`, at the tag of the asker's version, else `main`), picks the sections that match the question, and asks a small hosted model to answer from those sections only. The answer comes back with the sections it used, which the app names under it.

- **Model.** Workers AI, `@cf/zai-org/glm-4.7-flash` by default (`MODEL` in `wrangler.toml`; any Workers AI text model works, and the service reads both the `response` and the OpenAI `choices` answer shapes). It is Cloudflare's recommended replacement for the Llama 3.x models it is retiring. At 5,500 neurons per million input tokens and 36,400 per million output, a question costs about 30 neurons (up to about 75 with a long conversation), so the free 10,000 a day cover roughly 150 to 300 questions; past that a free account stops answering until the next day and a paid one is billed $0.011 per 1,000 neurons, so on a paid account set a budget alert under Billing. To answer through DeepSeek instead, set `PROVIDER = "deepseek"` in `[vars]` and `npx wrangler@4 secret put DEEPSEEK_API_KEY`.
- **What is sent.** The question, up to 6 earlier turns of the same conversation (4,000 characters in all), the app version and the share choice. Never anything from the project.
- **Nothing kept unless agreed.** The app sends `share: true` only when the person ticked the box. Shared questions (the question, the guide section it matched, the app version; no address) go to the Analytics Engine dataset `wugs_help_asked`, which keeps them three months. Unshared questions are not stored; Cloudflare's request logs (`[observability]`) keep request metadata, not bodies.
- **Guide changes need no deploy.** The guide is fetched from GitHub and kept, parsed, for an hour in each running copy of the Worker (the Cache API does nothing on workers.dev).
- **Limits.** 10 questions a minute per address and 20 a minute from everyone together (`EVERYONE`), so one abuser cannot spend the day's allocation in minutes. Questions up to 1,000 characters.

## Deploy

`.github/workflows/help.yml` deploys on every push to `main` that touches this folder, and on demand (Actions → help → Run workflow). It needs two repository secrets: `CLOUDFLARE_ACCOUNT_ID`, and `CLOUDFLARE_API_KEY` holding an **API token** (not the Global API Key) with *Workers Scripts: Edit*, *Workers AI: Read* and *Account Analytics: Read*. The account needs a workers.dev subdomain (Workers & Pages → your subdomain; a new account picks one on its first visit there), or the deploy fails. The run's summary shows the service's address, and the step fails if wrangler printed none. Put it in `ENDPOINT` in `apps/desktop/src-tauri/src/help.rs`; builds made after that answer questions, and until then the Help sheet says it is not connected yet.

By hand, from this folder: `npx wrangler@4 login`, then `npx wrangler@4 deploy`.

## The "most asked" report

Locally, with the questions: `CLOUDFLARE_ACCOUNT_ID=… CLOUDFLARE_API_TOKEN=… npm run most-asked -- 30` prints the guide sections people asked about most in the last 30 days, with recent questions under each. Actions → help → Run workflow with **report** ticked prints the section counts only: this repository is public, and people agreed to share their questions with the maintainers, not with everyone who can read a run summary.

## Run your own

Anyone can: fork, point `GUIDE_URL` at your guide, deploy with your own Cloudflare account, and set `ENDPOINT` in your build.

## Test

`npm test` runs the search against the real guide and the Worker with the model stubbed. CI runs it. The deploy pins wrangler 4.161.0.
