const path = require('path');
const programDir = path.join(__dirname, '..', '..', 'program');
const idlDir = path.join(__dirname, '..', '..', 'program');
const binaryInstallDir = path.join(__dirname, '.crates');

module.exports = {
  // idlGenerator: 'shank',
  programName: 'lazor_kit',
  idlDir,
  sdkDir: path.join(__dirname, 'src', 'generated'),
  binaryInstallDir,
  programDir,
  removeExistingIdl: false,
  binaryArgs: '-p DfmiYzJSaeW4yBinoAF6RNa14gGmhXHiX1DNUofkztY2',
};
