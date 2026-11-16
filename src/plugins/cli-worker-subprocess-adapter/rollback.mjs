// @story 04.4
// @plugin cli-worker-subprocess-adapter
import { parseCliArgs, rollbackPlugin } from "./package-plugin.mjs";

const opts = parseCliArgs(process.argv.slice(2));
const result = await rollbackPlugin(opts.dbPath, opts.projectId);
console.log(result);
