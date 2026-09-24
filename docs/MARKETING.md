# Rebuild public media

Screenshots use the production egui UI and explicitly synthetic demo telemetry.
The renderer starts no sampler, opens no native window, and sends no OS input.
Every screenshot says DEMO DATA; public captions also identify the example data.
Never substitute captures of a developer's desktop or real process/command list.

```powershell
./scripts/export-marketing.ps1 -SitePath C:/path/to/website/trontop
cd scripts/marketing
npm ci
npx playwright install chromium
node render-og.cjs C:/path/to/website/trontop
node check-site.cjs C:/path/to/website/trontop C:/path/to/review-output
```

`target/marketing/gallery.json` is the screenshot/theme manifest. The export
updates the marked gallery block in the existing product page, copies the 16
screenshots, 12 theme JSON files, logo and font notices. It does not publish.
`render-og.cjs` makes a 1200 x 630 card from the real screenshots.

Design direction: preserve the existing T mark and Tront typography. Dark/light
page tokens, teal accent, restrained motion and colorful app screenshots.
Design variance 6, motion 3, density 4. Existing image-generated branding and
production UI renders supply the imagery; no invented product UI or stock photos.

The page uses semantic HTML, plain CSS/JavaScript, keyboard-accessible previews,
downloadable theme JSON, a native dialog, local fonts and a no-JavaScript fallback.
The automated browser check covers three viewport widths and both page modes.

Dependency notices are regenerated separately:

```powershell
python scripts/generate-notices.py
```

Review updated notices when dependencies change. The product page and blog call
the combined Apache 2.0 + Commons Clause license **source available**, not open
source. Third-party components retain their own terms.
