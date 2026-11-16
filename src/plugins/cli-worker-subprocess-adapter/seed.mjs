// @story 04.4
// @plugin cli-worker-subprocess-adapter
import { parseCliArgs, seedPlugin } from "./package-plugin.mjs";

const opts = parseCliArgs(process.argv.slice(2));
const result = await seedPlugin(opts.dbPath, opts.projectId);
console.log(result);
