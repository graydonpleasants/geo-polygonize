const fs = require('fs');
const path = require('path');

const dirs = ['pkg-scalar', 'pkg-simd', 'pkg-threads'];

dirs.forEach(dir => {
    // __dirname is scripts/, so we need to go up one level
    const filePath = path.join(__dirname, '..', dir, 'geo_polygonize.js');
    if (fs.existsSync(filePath)) {
        let content = fs.readFileSync(filePath, 'utf8');

        const targetString = `// TODO we could test for more things here, like \`Set\`s and \`Map\`s.`;

        const replacementString = `// TODO we could test for more things here, like \`Set\`s and \`Map\`s.
    if (val instanceof Map) {
        let debug = '[';
        for (let [k, v] of val) {
            debug += debugString(k) + ' => ' + debugString(v) + ', ';
        }
        return 'Map(' + val.size + ') ' + debug + ']';
    }
    if (val instanceof Set) {
        let debug = '[';
        for (let v of val) {
            debug += debugString(v) + ', ';
        }
        return 'Set(' + val.size + ') ' + debug + ']';
    }`;

        // Ensure it hasn't already been patched
        if (content.includes(targetString) && !content.includes(`if (val instanceof Map)`)) {
            content = content.replace(targetString, replacementString);
            fs.writeFileSync(filePath, content, 'utf8');
            console.log(`Patched ${filePath}`);
        } else if (content.includes(`if (val instanceof Map)`)) {
            console.log(`Already patched ${filePath}`);
        } else {
            console.log(`Target string not found in ${filePath}`);
        }
    } else {
        console.log(`File not found: ${filePath}`);
    }
});
