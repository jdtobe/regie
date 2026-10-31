// Offline source SVG -> native assets. Requires the pinned test WebKit and macOS iconutil.
const fs = require('node:fs/promises');
const path = require('node:path');
const os = require('node:os');
const {execFileSync} = require('node:child_process');
const {webkit} = require('@playwright/test');

async function main() {
  const root = path.resolve(__dirname, '..');
  const icons = path.join(root, 'assets/icons');
  const temporary = await fs.mkdtemp(path.join(os.tmpdir(), 'regie-icons-'));
  const browser = await webkit.launch();
  try {
    const page = await browser.newPage({deviceScaleFactor:1});
    await page.route('**/*', route => route.abort());
    async function render(source, size, destination) {
      const svg = await fs.readFile(path.join(icons, source), 'utf8');
      await page.setViewportSize({width:size,height:size});
      await page.setContent(`<style>html,body{margin:0;background:transparent}svg{display:block;width:100vw;height:100vh}</style>${svg}`);
      await page.screenshot({path:destination,omitBackground:true});
    }
    await render('menubar.svg',22,path.join(icons,'menubar-22.png'));
    await render('menubar.svg',44,path.join(icons,'menubar-44.png'));
    await render('app.svg',1024,path.join(icons,'app-1024.png'));
    const iconset = path.join(temporary,'Regie.iconset');
    await fs.mkdir(iconset);
    for(const base of [16,32,128,256,512]) for(const scale of [1,2]) {
      await render('app.svg',base*scale,path.join(iconset,`icon_${base}x${base}${scale===2?'@2x':''}.png`));
    }
    execFileSync('iconutil',['-c','icns',iconset,'-o',path.join(icons,'Regie.icns')]);
    // Inspection sizes, not additional shipped assets.
    for(const size of [16,32,48,128,512]) await render('app.svg',size,path.join(temporary,`preview-${size}.png`));
    console.log('Regenerated Regie PNG masters and complete ICNS.');
  } finally {
    await browser.close();
    await fs.rm(temporary,{recursive:true,force:true});
  }
}
main().catch(error=>{console.error(error);process.exitCode=1;});
