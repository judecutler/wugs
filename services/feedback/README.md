# wugs feedback service

The app's **Feedback** button sends reports here. This small Cloudflare Worker:

1. keeps each report in an R2 bucket,
2. files it as an issue in a **private** GitHub repository (label `feedback`),
3. emails the maintainer through [Resend](https://resend.com), with a reply-to address when the person gave one,
4. takes the optional screenshot or video (up to 95 MB) in a second request, streams it into R2, and comments on the issue with a private download link.

Each report carries an id the app chose, and every step is idempotent on it. An app that lost an answer sends the same report again, and nothing is filed twice. The person's words go into the issue inside a code fence, so @mentions and links in them do nothing.

Anyone can run their own copy for free; nothing here is specific to the wugs maintainers except the names below.

## Set up (once, about 15 minutes)

You need Node.js 22 and this folder.

1. **A private repository for the reports.** On GitHub, create `wugs-feedback` (or any name) and make it **Private**. Put its `owner/name` in `FEEDBACK_REPO` in `wrangler.toml`.
2. **A GitHub token that can only write issues there.** GitHub → Settings → Developer settings → Fine-grained tokens → Generate new token. Under Repository access, choose **Only select repositories** and pick the private repository. Under Repository permissions, set **Issues: Read and write**. Copy the token.
3. **An email sender.** Make a free [Resend](https://resend.com) account with the email address that should receive the reports, then create an API key. Without your own domain, Resend sends only to your account's own address, which is all this needs.
4. **Storage.** In the Cloudflare dashboard, open **R2**. Cloudflare asks for a payment method the first time; the free tier (10 GB) is not charged. Create a bucket named `wugs-feedback`.
5. **Sign in and deploy.** In this folder:

   ```
   npx wrangler@4 login
   npx wrangler@4 secret put LINK_SECRET      # paste any long random text
   npx wrangler@4 secret put GITHUB_TOKEN     # the token from step 2
   npx wrangler@4 secret put RESEND_API_KEY   # the key from step 3
   npx wrangler@4 secret put OWNER_EMAIL      # where reports go
   npx wrangler@4 deploy
   ```

   The last command prints the service's address, such as `https://wugs-feedback.<your-name>.workers.dev`.
6. **Point the app at it.** Put that address in `ENDPOINT` in `apps/desktop/src-tauri/src/feedback.rs`. Builds made after that send feedback; until then the app keeps reports on the computer and says so.

`LINK_SECRET` signs upload and download links. Changing it breaks links already posted in issues.

## Day to day

- New reports arrive as an email and as an issue in the private repository. Reply to the email to answer the person, when they left an address.
- Attachment links work for anyone who has them, so keep them inside the private repository. To remove a report's files, delete `reports/<id>/` in the R2 bucket.
- At most 10 reports or uploads a minute come from one address (the `[[ratelimits]]` block).

## Limits

- 95 MB per attachment: Cloudflare's free plan refuses larger requests.
- Workers on the free plan take 100,000 requests a day, far more than feedback needs.
- Email and GitHub are each skipped when their secrets are unset, so either can be turned off.

## Test

`npm test` runs the Worker against an in-memory bucket with GitHub and email stubbed. CI runs it.
