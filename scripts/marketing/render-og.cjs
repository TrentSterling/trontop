// node render-og.cjs <product-page-directory>
const { chromium } = require('playwright');
const { resolve, join } = require('node:path');
const { readFileSync } = require('node:fs');
(async () => {
  const site = resolve(process.argv[2]);
  const src = name => `data:image/png;base64,${readFileSync(join(site, 'media', name)).toString('base64')}`;
  const font = `data:font/ttf;base64,${readFileSync(join(site,'media/Rajdhani-SemiBold.ttf')).toString('base64')}`;
  const browser = await chromium.launch({headless:true});
  try {
    const page = await browser.newPage({viewport:{width:1200,height:630},deviceScaleFactor:1});
    await page.setContent(`<!doctype html><html><head><style>
      @font-face{font-family:Rajdhani;src:url('${font}')}*{box-sizing:border-box}body{margin:0;background:#101419;color:#f3f4f5;width:1200px;height:630px;overflow:hidden;font-family:'Segoe UI',sans-serif}.copy{position:absolute;left:55px;top:46px;width:480px}.brand{display:flex;align-items:center;gap:12px;font-size:29px;letter-spacing:2px}.brand img{width:49px;height:49px;object-fit:contain}h1{font-family:Rajdhani,sans-serif;font-size:76px;line-height:1.02;letter-spacing:-2.8px;font-weight:650;margin:39px 0 25px}h1 span{color:#99f0d7}p{color:#bac6cf;font-size:21px;line-height:1.6;max-width:420px}.url{position:absolute;bottom:45px;left:55px;color:#99f0d7;font-size:20px}.shots{position:absolute;left:554px;top:58px;width:670px}.shots img{position:absolute;width:630px;border:1px solid #53616f;border-radius:9px;box-shadow:0 20px 35px #0007}.one{top:0;left:58px}.two{top:140px;left:24px}.three{top:280px;left:-10px}.demo{position:absolute;right:26px;bottom:15px;color:#acb9c6;font-size:11px}
      </style></head><body><div class="copy"><div class="brand"><img src="${src('logo.png')}" alt="">TRONTOP</div><h1>Your PC.<br><span>In full color.</span></h1><p>A native Windows task manager<br>with a serious color habit.</p></div><div class="shots"><img class="one" src="${src('daylight-overview.png')}" alt=""><img class="two" src="${src('spectrum-overview.png')}" alt=""><img class="three" src="${src('electric-overview.png')}" alt=""></div><div class="url">tront.xyz/trontop</div><div class="demo">App screenshots use demo data</div></body></html>`);
    await page.evaluate(async () => { await document.fonts.ready; await Promise.all([...document.images].map(i=>i.decode())); });
    await page.screenshot({path:join(site,'media/og.png')});
    console.log('Rendered media/og.png (1200 x 630) from real app screenshots.');
  } finally { await browser.close(); }
})().catch(error=>{console.error(error);process.exitCode=1;});
