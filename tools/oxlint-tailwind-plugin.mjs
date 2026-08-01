import { readFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { __unstable__loadDesignSystem } from "@tailwindcss/node";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const stylesheet = path.join(root, "apps/desktop/src/index.css");
const css = await readFile(stylesheet, "utf8");
const designSystem = await __unstable__loadDesignSystem(css, {
  base: path.dirname(stylesheet),
});

const classHelpers = new Set(["cn", "clsx", "classnames", "cva", "twMerge"]);

function isClassAttribute(node) {
  return (
    node?.type === "JSXAttribute" &&
    node.name?.type === "JSXIdentifier" &&
    (node.name.name === "className" || node.name.name === "class")
  );
}

function isClassHelperCall(node) {
  return (
    node?.type === "CallExpression" &&
    node.callee?.type === "Identifier" &&
    classHelpers.has(node.callee.name)
  );
}

function carriesClasses(node) {
  for (let parent = node.parent; parent; parent = parent.parent) {
    if (isClassAttribute(parent) || isClassHelperCall(parent)) return true;
    if (parent.type === "Program" || parent.type === "BlockStatement") return false;
  }
  return false;
}

function canonicalize(value) {
  return value.replace(/\S+/g, (candidate) => {
    const [canonical] = designSystem.canonicalizeCandidates([candidate], { rem: 16 });
    return canonical ?? candidate;
  });
}

const canonicalClasses = {
  meta: {
    type: "suggestion",
    docs: {
      description: "Require Tailwind's canonical utility class names",
    },
    fixable: "code",
    schema: [],
    messages: {
      canonical: "Use canonical Tailwind classes: {{replacement}}",
    },
  },
  create(context) {
    function report(node, value, replacement, output) {
      if (replacement === value) return;

      context.report({
        node,
        messageId: "canonical",
        data: { replacement },
        fix(fixer) {
          return fixer.replaceText(node, output(replacement));
        },
      });
    }

    return {
      Literal(node) {
        if (typeof node.value !== "string" || !carriesClasses(node)) return;
        const replacement = canonicalize(node.value);
        report(node, node.value, replacement, JSON.stringify);
      },
      TemplateLiteral(node) {
        if (node.expressions.length !== 0 || !carriesClasses(node)) return;
        const value = node.quasis[0]?.value.cooked;
        if (typeof value !== "string") return;
        const replacement = canonicalize(value);
        report(node, value, replacement, (next) => `\`${next.replaceAll("`", "\\`")}\``);
      },
    };
  },
};

export default {
  meta: {
    name: "nirmoka-tailwind",
  },
  rules: {
    "canonical-classes": canonicalClasses,
  },
};
