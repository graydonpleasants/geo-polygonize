import fs from 'fs';
import path from 'path';

const MANIFEST_PATH = 'examples/manifest.json';
const EXAMPLES_DIR = 'docs/examples';

if (!fs.existsSync(MANIFEST_PATH)) {
    console.error(`Manifest not found at ${MANIFEST_PATH}`);
    process.exit(0);
}

const manifest = JSON.parse(fs.readFileSync(MANIFEST_PATH, 'utf8'));

fs.mkdirSync(EXAMPLES_DIR, { recursive: true });

let indexMd = `# Examples Catalog\n\n`;

for (const scenario of manifest) {
    indexMd += `* [${scenario.title}](./${scenario.slug}.md) - ${scenario.description}\n`;

    let pageMd = `# ${scenario.title}\n\n`;
    pageMd += `${scenario.description}\n\n`;
    pageMd += `## Configuration\n\n\`\`\`json\n${JSON.stringify(scenario.defaultOptions, null, 2)}\n\`\`\`\n\n`;

    const fixturePath = path.join('examples', scenario.fixture);
    if (fs.existsSync(fixturePath)) {
        const fixtureContent = fs.readFileSync(fixturePath, 'utf8');
        pageMd += `## Input Geometry\n\n\`\`\`json\n${fixtureContent}\n\`\`\`\n\n`;
    }

    pageMd += `## Interactive Playground\n\n`;
    pageMd += `You can experiment with this scenario in the [Playground](/playground/?scenario=${scenario.slug}).\n`;

    fs.writeFileSync(path.join(EXAMPLES_DIR, `${scenario.slug}.md`), pageMd);
}

fs.writeFileSync(path.join(EXAMPLES_DIR, 'index.md'), indexMd);
console.log(`Generated ${manifest.length} example pages.`);
