const fs = require('fs');
const path = require('path');

const filePath = process.argv[2];
if (!filePath) {
    console.error('Usage: node patch_debug.cjs <file-path>');
    process.exit(1);
}

let content = fs.readFileSync(filePath, 'utf8');

const patch = `    if (val instanceof Map) {
        let debug = 'Map(' + val.size + ') {';
        let first = true;
        for (const [key, value] of val.entries()) {
            if (!first) {
                debug += ', ';
            }
            debug += debugString(key) + ' => ' + debugString(value);
            first = false;
        }
        debug += '}';
        return debug;
    }
    if (val instanceof Set) {
        let debug = 'Set(' + val.size + ') {';
        let first = true;
        for (const value of val.values()) {
            if (!first) {
                debug += ', ';
            }
            debug += debugString(value);
            first = false;
        }
        debug += '}';
        return debug;
    }`;

const todoComment = '    // TODO we could test for more things here, like `Set`s and `Map`s.';

if (content.includes(todoComment)) {
    content = content.replace(todoComment, patch);
    fs.writeFileSync(filePath, content);
    console.log('Successfully patched ' + filePath);
} else if (content.includes('Set(') && content.includes('Map(')) {
    console.log('File ' + filePath + ' already appears to be patched.');
} else {
    console.error('Could not find TODO comment in ' + filePath + '. Patch failed.');
    process.exit(1);
}
