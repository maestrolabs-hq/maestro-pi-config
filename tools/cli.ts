import process from "node:process";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";
import { restore, status, sync } from "./config.ts";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");

const SYMBOL: Record<string, string> = {
  same: "  ok    ",
  differs: "  DRIFT ",
  "absent-live": "  absent",
  "absent-repo": "  NEW   ",
};

function reportStatus(): number {
  const reports = status(root);
  let drifted = 0;
  for (const r of reports) {
    console.log(`${SYMBOL[r.state]} ${r.entry.repo}`);
    for (const d of r.detail) console.log(`          ${d}`);
    if (r.state === "differs") drifted += 1;
  }
  console.log(
    drifted === 0
      ? "\nIn sync with this machine."
      : `\n${drifted} entr${drifted === 1 ? "y has" : "ies have"} drifted. \`just sync\` pulls the machine's version in.`,
  );
  return drifted === 0 ? 0 : 1;
}

function main(argv: readonly string[]): number {
  const [verb, ...rest] = argv;
  switch (verb) {
    case "status":
      return reportStatus();
    case "sync": {
      const written = sync(root);
      for (const w of written) console.log(`  wrote ${w}`);
      console.log(`\n${written.length} file(s) pulled in. Review with \`git diff\`.`);
      return 0;
    }
    case "restore": {
      const apply = rest.includes("--apply");
      const touched = restore(root, apply);
      for (const t of touched) console.log(`  ${apply ? "wrote" : "would write"} ${t}`);
      console.log(
        apply
          ? `\n${touched.length} file(s) restored.`
          : `\n${touched.length} file(s) would be written. Nothing changed. Re-run with --apply.`,
      );
      return 0;
    }
    default:
      console.error("usage: config <status|sync|restore [--apply]>");
      return 2;
  }
}

process.exitCode = main(process.argv.slice(2));
