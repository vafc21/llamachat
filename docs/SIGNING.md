# Publishing & signing LlamaChat

LlamaChat is open source (Apache-2.0) and the code repo is already public. This
page is about **distributing the built app** — letting people download and run
installers — and how to make those downloads **code-signed** so Windows/macOS
don't show "unknown developer" warnings.

There are two paths, and you can start with the free/backup one today and add
signing later:

1. **Unsigned (the backup — free, works now).** Publish installers; users click
   past a one-time warning. Normal for open-source apps.
2. **Signed (the goal — also free for open source, via SignPath Foundation).**
   Removes the warnings.

---

## 1. Publishing releases (works today, unsigned)

The build workflow (`.github/workflows/build.yml`) publishes a **GitHub Release**
with macOS/Windows/Linux installers attached whenever you push a version tag:

```bash
git tag v0.1.0
git push origin v0.1.0
```

The installers then appear at
`https://github.com/vafc21/llamachat/releases` for anyone to download.

Because they're unsigned, on first launch:

- **Windows:** SmartScreen shows *"Windows protected your PC"* → the user clicks
  **More info → Run anyway**. (On machines with Smart App Control enforced, they
  may first need to right-click the file → **Properties → Unblock**.)
- **macOS:** the user right-clicks the app → **Open** (once).

Put a short note about this in your README so users aren't surprised — it's
standard for open-source software.

---

## 2. Free code signing for open source: SignPath Foundation

[SignPath Foundation](https://signpath.org/) gives qualifying open-source
projects a code-signing certificate **and** a cloud signing service at **no
cost**. Signed installers download and run **without** the Windows warnings
(including on Smart App Control machines).

How to turn it on:

1. **Apply** at <https://signpath.org/> — they verify the project is genuinely
   open source. Approval takes a few days.
2. Once approved, in the SignPath dashboard you'll have an **organization ID**,
   and you create a **project** (use slug `llamachat`) and a **signing policy**
   (e.g. `release-signing`) plus a **CI user API token**.
3. In GitHub → your repo → **Settings**:
   - **Secrets and variables → Actions → Secrets:** add `SIGNPATH_API_TOKEN`.
   - **Secrets and variables → Actions → Variables:** add
     `SIGNPATH_ORGANIZATION_ID`.
4. In `.github/workflows/build.yml`, **uncomment the SignPath step** in the
   `release` job (it's already stubbed in with the right shape) and adjust the
   `project-slug` / `signing-policy-slug` to match what you created.
5. Push a new tag — the released **Windows** installer is now signed.

That's it: warning-free downloads, no cost.

---

## Alternative: Azure Trusted Signing (~$10/month)

If you'd rather not wait on the SignPath application, Microsoft's **Azure Trusted
Signing** signs during the build itself. Add a `bundle.windows.signCommand` to
`tauri.conf.json` (using the `trusted-signing-cli` tool) and set the Azure
service-principal secrets (`AZURE_CLIENT_ID` / `AZURE_CLIENT_SECRET` /
`AZURE_TENANT_ID`) in CI. See
<https://learn.microsoft.com/azure/trusted-signing/>.

## macOS notarization (optional)

`tauri.conf.json` currently signs with an Apple *Development* identity for local
builds. For warning-free public macOS downloads you need a **Developer ID
Application** certificate + **notarization** (Apple Developer Program, $99/yr).
Tauri notarizes during `cargo tauri build` when the `APPLE_ID`,
`APPLE_PASSWORD`, and `APPLE_TEAM_ID` env vars are set in CI.
