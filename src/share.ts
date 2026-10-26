export const SHARE_TEXT = '星镜 Starshot：本地截图、标注、置顶贴图与图片美化。渐变背景、圆角边框、离线敏感信息打码，让截图更清楚、更好看。图片在本机处理，不上传。macOS / Windows。\nhttps://github.com/cpython666/starshot';

/** Original vector-like product artwork, drawn locally without reading the desktop. */
export function createShareImage(): string {
  const c = document.createElement('canvas'); c.width = 1080; c.height = 1440;
  const x = c.getContext('2d'); if (!x) throw Error('无法生成产品介绍图');
  const gradient = x.createLinearGradient(0,0,1080,1440); gradient.addColorStop(0,'#171527'); gradient.addColorStop(1,'#283654'); x.fillStyle=gradient; x.fillRect(0,0,1080,1440);
  x.fillStyle='#c8b3fb'; x.font='600 32px sans-serif'; x.fillText('✦  STARSHOT',80,112);
  x.fillStyle='#ffffff'; x.font='700 92px sans-serif'; x.fillText('星镜',80,245);
  x.font='600 48px sans-serif'; x.fillText('本地截图与图片美化',80,335);
  x.fillStyle='#c2bed5'; x.font='30px sans-serif'; x.fillText('截图 · 标注 · 置顶贴图 · 敏感信息打码',80,400);
  const g=x.createLinearGradient(80,480,1000,1010);g.addColorStop(0,'#b5a0e2');g.addColorStop(1,'#617aaa');x.fillStyle=g;x.beginPath();x.roundRect(80,480,920,530,32);x.fill();
  x.fillStyle='#fbfaff';x.beginPath();x.roundRect(150,550,780,390,20);x.fill();
  x.fillStyle='#383346';x.font='600 35px sans-serif';x.fillText('把灵感，定格成星。',200,630);
  x.fillStyle='#e4e0ed';x.fillRect(200,665,590,12);x.fillRect(200,702,420,12);
  x.fillStyle='#1b1c28';x.fillRect(200,765,280,34);
  x.strokeStyle='#a985d5';x.lineWidth=7;x.strokeRect(188,753,305,58);
  x.beginPath();x.moveTo(690,845);x.lineTo(540,800);x.lineTo(570,834);x.moveTo(540,800);x.lineTo(584,790);x.stroke();
  x.fillStyle='#ffffff';x.font='600 35px sans-serif';x.fillText('图片只在本机处理，不上传',80,1110);
  x.fillStyle='#c2bed5';x.font='28px sans-serif';x.fillText('原创示意画面 · macOS / Windows',80,1170);
  x.font='25px sans-serif';x.fillText('github.com/cpython666/starshot',80,1320);
  // Six-bit RGB keeps this generated poster compact while retaining softer gradients.
  const pixels = x.getImageData(0, 0, c.width, c.height);
  for (let i = 0; i < pixels.data.length; i += 4) {
    pixels.data[i] &= 252;
    pixels.data[i + 1] &= 252;
    pixels.data[i + 2] &= 252;
  }
  x.putImageData(pixels, 0, 0);
  return c.toDataURL('image/png');
}
