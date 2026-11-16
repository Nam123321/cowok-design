// @story 04.3
// @plugin cli-doctor-extension
import { parseCliArgs, seedPlugin } from "./package-plugin.mjs";

const opts = parseCliArgs(process.argv.slice(2));
const result = await seedPlugin(opts.dbPath);
console.log(JSON.stringify(result));
