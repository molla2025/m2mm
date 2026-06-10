const sharp = require('sharp');
const fs = require('fs');
const path = require('path');
const { default: pngToIco } = require('png-to-ico');

const iconSizes = [32, 128, 256, 512];
const icoSizes = [16, 32, 48, 256]; // Windows ICO standard sizes
const iconsDir = path.join(__dirname, 'src-tauri', 'icons');

// Create icons directory if it doesn't exist
if (!fs.existsSync(iconsDir)) {
  fs.mkdirSync(iconsDir, { recursive: true });
}

const inputIcon = path.join(__dirname, 'icon.png');

async function generateIcons() {
  try {
    console.log('🎨 Generating icons...');

    // Generate PNG icons
    for (const size of iconSizes) {
      const outputPath = path.join(iconsDir, `${size}x${size}.png`);
      await sharp(inputIcon)
        .resize(size, size, {
          fit: 'contain',
          background: { r: 0, g: 0, b: 0, alpha: 0 }
        })
        .png()
        .toFile(outputPath);
      
      console.log(`✅ Generated ${size}x${size}.png`);
    }

    // Generate 128@2x for retina
    const retina128 = path.join(iconsDir, '128x128@2x.png');
    await sharp(inputIcon)
      .resize(256, 256, {
        fit: 'contain',
        background: { r: 0, g: 0, b: 0, alpha: 0 }
      })
      .png()
      .toFile(retina128);
    console.log(`✅ Generated 128x128@2x.png`);

    // Generate icon.png (1024x1024 for general use)
    const iconPng = path.join(iconsDir, 'icon.png');
    await sharp(inputIcon)
      .resize(1024, 1024, {
        fit: 'contain',
        background: { r: 0, g: 0, b: 0, alpha: 0 }
      })
      .png()
      .toFile(iconPng);
    console.log(`✅ Generated icon.png`);

    // Generate ICO-specific PNG files with proper sizes
    console.log('🔧 Generating ICO component PNGs...');
    const icoPngPaths = [];
    for (const size of icoSizes) {
      const tempPath = path.join(iconsDir, `temp_${size}.png`);
      await sharp(inputIcon)
        .resize(size, size, {
          fit: 'contain',
          background: { r: 0, g: 0, b: 0, alpha: 0 }
        })
        .png()
        .toFile(tempPath);
      icoPngPaths.push(tempPath);
    }

    // Generate proper ICO file with multiple sizes (16, 32, 48, 256)
    console.log('🔨 Creating ICO file...');
    const iconIco = path.join(iconsDir, 'icon.ico');
    const icoBuffer = await pngToIco(icoPngPaths);
    fs.writeFileSync(iconIco, icoBuffer);
    
    // Clean up temporary PNG files
    for (const tempPath of icoPngPaths) {
      fs.unlinkSync(tempPath);
    }
    
    console.log(`✅ Generated icon.ico (16x16, 32x32, 48x48, 256x256)`);

    // For macOS ICNS, just use the largest PNG
    const iconIcns = path.join(iconsDir, 'icon.icns');
    await sharp(inputIcon)
      .resize(1024, 1024, {
        fit: 'contain',
        background: { r: 0, g: 0, b: 0, alpha: 0 }
      })
      .png()
      .toFile(iconIcns);
    console.log(`✅ Generated icon.icns (as PNG, macOS compatible)`);

    console.log('🎉 All icons generated successfully!');
  } catch (error) {
    console.error('❌ Error generating icons:', error);
    process.exit(1);
  }
}

generateIcons();