// const fs = require("node:fs");
const { join } = require("node:path");
console.log("pre-load");
const sharp = require("sharp");
console.log("post-load");

const inputPath = join(__dirname, "../icon.svg");
const outputPath = join(__dirname, "../icon.png");
console.log("pre-render");
sharp(inputPath).resize(128, 128).png().toFile(outputPath);
