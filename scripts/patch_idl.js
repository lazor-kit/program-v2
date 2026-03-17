const fs = require('fs');
const path = require('path');

const idlPath = path.join(__dirname, '..', 'program', 'lazor_kit.json');
const idl = JSON.parse(fs.readFileSync(idlPath, 'utf-8'));

console.log('--- 🛠 Patching IDL for Runtime Alignments ---');

// 1. Inject program address (missing from Shank IDL)
idl.metadata = idl.metadata || {};
idl.metadata.address = 'FLb7fyAtkfA4TSa2uYcAT8QKHd2pkoMHgmqfnXFXo7ao';

// 3. Cast [u8; 32] fields to publicKey for Accounts
if (idl.accounts) {
    idl.accounts.forEach(acc => {
        if (acc.type && acc.type.fields) {
            acc.type.fields.forEach(f => {
                if (['wallet', 'sessionKey'].includes(f.name)) {
                    f.type = 'publicKey';
                }
            });
        }
    });
}

fs.writeFileSync(idlPath, JSON.stringify(idl, null, 2));
console.log('✓ Successfully patched lazor_kit.json');
