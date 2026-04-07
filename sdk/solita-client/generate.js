const { Solita } = require('@metaplex-foundation/solita');
const path = require('path');
const fs = require('fs');

const idlPath = path.join(__dirname, '..', '..', 'program', 'lazor_kit.json');

if (!fs.existsSync(idlPath)) {
    console.error(`❌ IDL not found at ${idlPath}`);
    process.exit(1);
}

const idl = JSON.parse(fs.readFileSync(idlPath, 'utf8'));

// Ensure metadata.address is set (or fallback to something)
if (!idl.metadata || !idl.metadata.address) {
    console.warn(`⚠️ Warning: idl.metadata.address is missing. Setting default.`);
    idl.metadata = idl.metadata || {};
    idl.metadata.address = 'DfmiYzJSaeW4yBinoAF6RNa14gGmhXHiX1DNUofkztY2';
}

const outputDir = path.join(__dirname, 'src', 'generated');

console.log(`--- 🔨 Generating Solita SDK from ${idlPath} ---`);
console.log(`Output: ${outputDir}`);

const solita = new Solita(idl, {
    formatCode: true,
});

solita.renderAndWriteTo(outputDir)
    .then(() => {
        console.log('--- ✅ Solita SDK Generated Successfully! ---');
        process.exit(0);
    })
    .catch(err => {
        console.error('❌ Generation Failed:', err);
        process.exit(1);
    });
