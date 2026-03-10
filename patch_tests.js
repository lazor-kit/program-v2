const fs = require('fs');
const path = require('path');

const dir = 'tests-real-rpc/tests';
const files = fs.readdirSync(dir)
    .filter(file => file.endsWith('.test.ts'))
    .map(file => path.join(dir, file));

files.forEach(file => {
    let content = fs.readFileSync(file, 'utf8');

    const methods = [
        'createWallet',
        'buildExecute',
        'addAuthority',
        'removeAuthority',
        'transferOwnership',
        'execute',
        'createSession'
    ];

    methods.forEach(method => {
        const regex = new RegExp(`client\\.${method}\\(\\{`, 'g');
        content = content.replace(regex,
            `client.${method}({\n            config: context.configPda,\n            treasuryShard: context.treasuryShard,`
        );
    });

    fs.writeFileSync(file, content);
    console.log(`Patched ${file}`);
});
