#!/usr/bin/env python3
"""Generate rust/crates/subtrack/src/commands.rs from the TS command definitions.

Reads the TypeScript reference implementation's command files
(packages/subtrack/src/commands/*.ts) and emits a Rust static registry:
top-level subcommands + every nested subcommand, with args (help rendering
surface: type/short/description/array/required), toKebab, and subcommand
index references into a flat COMMANDS table.

Generated file is committed; regenerate with:
  python3 rust/scripts/gen_commands.py
"""

import os
import re
import sys

SRC = "/tmp/opencode/tagwt/packages/subtrack/src/commands"
OUT = os.path.join(
    os.path.dirname(os.path.dirname(os.path.abspath(__file__))),
    "crates",
    "subtrack",
    "src",
    "commands.rs",
)

FILES = [
    "core.ts",
    "tag.ts",
    "trial.ts",
    "bulk.ts",
    "io.ts",
    "backup.ts",
    "config.ts",
    "usage.ts",
    "report.ts",
    "misc.ts",
    "suggest.ts",
    "features.ts",
]

# ── Lexer ─────────────────────────────────────────────────────────

PUNCT = set("{}[]():,=;")


def lex(src):
    """Tokenize TS source into (kind, value) tokens; strings decoded."""
    tokens = []
    i, n = 0, len(src)
    while i < n:
        c = src[i]
        # whitespace
        if c in " \t\r\n":
            i += 1
            continue
        # line comment
        if c == "/" and i + 1 < n and src[i + 1] == "/":
            j = src.find("\n", i)
            i = n if j == -1 else j + 1
            continue
        # block comment
        if c == "/" and i + 1 < n and src[i + 1] == "*":
            j = src.find("*/", i + 2)
            i = n if j == -1 else j + 2
            continue
        # string literals
        if c in "'\"":
            quote = c
            i += 1
            buf = []
            while i < n:
                ch = src[i]
                if ch == "\\":
                    if i + 1 >= n:
                        break
                    esc = src[i + 1]
                    mapping = {
                        "n": "\n",
                        "t": "\t",
                        "r": "\r",
                        "b": "\b",
                        "f": "\f",
                        "0": "\0",
                        "\\": "\\",
                        "'": "'",
                        '"': '"',
                    }
                    buf.append(mapping.get(esc, esc))
                    i += 2
                    continue
                if ch == quote:
                    i += 1
                    break
                buf.append(ch)
                i += 1
            tokens.append(("str", "".join(buf)))
            continue
        # template literal
        if c == "`":
            i += 1
            buf = []
            while i < n:
                ch = src[i]
                if ch == "\\":
                    buf.append(src[i : i + 2])
                    i += 2
                    continue
                if ch == "`":
                    i += 1
                    break
                buf.append(ch)
                i += 1
            tokens.append(("str", "".join(buf)))
            continue
        # two-char operator => (before punct so `=` isn't consumed alone)
        if c == "=" and i + 1 < n and src[i + 1] == ">":
            tokens.append(("=>", "=>"))
            i += 2
            continue
        # punctuation
        if c in PUNCT:
            tokens.append((c, c))
            i += 1
            continue
        # identifiers / numbers
        m = re.match(r"[A-Za-z_$][A-Za-z0-9_$]*|\d+(?:\.\d+)?", src[i:])
        if m:
            tokens.append(("id", m.group(0)))
            i += len(m.group(0))
            continue
        # unknown single char (spread ... etc.) — skip
        i += 1
    return tokens


# ── Parser ─────────────────────────────────────────────────────────

SKIP = object()


def parse_value(toks, pos):
    """Parse one value from the token list. Returns (value, newpos)."""
    kind, val = toks[pos]
    if kind == "str":
        return val, pos + 1
    if kind == "{":
        return parse_object(toks, pos + 1)
    if kind == "(":
        # zero-param arrow: `() => expr` / `() => ({...})`
        after = skip_balanced(toks, pos, "(", ")")
        if after < len(toks) and toks[after][0] == "=>":
            return skip_arrow_expr(toks, after + 1)
        raise ValueError(f"unexpected '(' at {pos}")
    if kind == "[":
        return parse_array(toks, pos + 1)
    if kind == "id":
        if val == "true":
            return True, pos + 1
        if val == "false":
            return False, pos + 1
        if val == "null" or val == "undefined":
            return None, pos + 1
        if val == "async" or val == "function":
            return skip_function(toks, pos + 1)
        # arrow with bare param: `x => expr`
        if pos + 1 < len(toks) and toks[pos + 1][0] == "=>":
            return skip_arrow_expr(toks, pos + 2)
        # const reference (subCommands values) or plain identifier
        return ("ref", val), pos + 1
    if kind == "number":
        return float(val), pos + 1
    raise ValueError(f"unexpected token {kind}={val!r}")


def parse_object(toks, pos):
    obj = {}
    order = []
    while pos < len(toks):
        kind, val = toks[pos]
        if kind == "}":
            return obj, pos + 1
        if kind == ",":
            pos += 1
            continue
        if kind == "id" or kind == "str":
            key = val
            pos += 1
            if pos < len(toks) and toks[pos][0] == ":":
                pos += 1
                value, pos = parse_value(toks, pos)
                # skip `as <Type>` casts (e.g. `"positional" as const`)
                if pos < len(toks) and toks[pos][0] == "id" and toks[pos][1] == "as":
                    pos += 2 if pos + 1 < len(toks) else len(toks)
                if key not in obj:
                    order.append(key)
                obj[key] = value
                continue
            raise ValueError(f"expected ':' after key {key!r}")
        raise ValueError(f"unexpected token in object {kind}={val!r}")
    raise ValueError("unterminated object")


def parse_array(toks, pos):
    arr = []
    while pos < len(toks):
        kind, val = toks[pos]
        if kind == "]":
            return arr, pos + 1
        if kind == ",":
            pos += 1
            continue
        value, pos = parse_value(toks, pos)
        arr.append(value)
    raise ValueError("unterminated array")


def skip_balanced(toks, pos, open_tok, close_tok):
    depth = 0
    while pos < len(toks):
        kind, val = toks[pos]
        if kind == open_tok:
            depth += 1
        elif kind == close_tok:
            depth -= 1
            if depth == 0:
                return pos + 1
        pos += 1
    raise ValueError("unbalanced")


def skip_function(toks, pos):
    """After `async`/`function`: name? params (..) => body/expr."""
    if pos < len(toks) and toks[pos][0] == "id" and toks[pos][1] != "(":
        pos += 1  # function name
    if pos < len(toks) and toks[pos][0] == "(":
        pos = skip_balanced(toks, pos, "(", ")")
    if pos < len(toks) and toks[pos][0] == "=>":
        return skip_arrow_expr(toks, pos + 1)
    # bare function without arrow (rare) — assume block or expr
    if pos < len(toks) and toks[pos][0] == "{":
        return SKIP, skip_balanced(toks, pos, "{", "}")
    return SKIP, pos


def skip_arrow_expr(toks, pos):
    if pos < len(toks) and toks[pos][0] == "{":
        return SKIP, skip_balanced(toks, pos, "{", "}")
    # expression body: consume until top-level ',' or '}' (inside object) / ']'
    depth = 0
    while pos < len(toks):
        kind, val = toks[pos]
        if kind in ("(", "[", "{"):
            depth += 1
        elif kind in (")", "]", "}"):
            if depth == 0:
                return SKIP, pos
            depth -= 1
        elif depth == 0 and kind == ",":
            return SKIP, pos
        pos += 1
    raise ValueError("unterminated arrow expr")


# ── Extraction ─────────────────────────────────────────────────────


def find_defs(tokens):
    """Find define({...}) calls bound to a const name: `const X = define(`."""
    defs = {}  # const name -> def dict
    i = 0
    while i < len(tokens):
        # look for `<const|export const> <name> = define(`
        if tokens[i][0] == "id" and tokens[i][1] == "const":
            j = i + 1
            if j < len(tokens) and tokens[j][0] == "id":
                name = tokens[j][1]
            else:
                i += 1
                continue
            if (
                tokens[j + 1][0] == "="
                and j + 2 < len(tokens)
                and tokens[j + 2][0] == "id"
                and tokens[j + 2][1] == "define"
                and j + 3 < len(tokens)
                and tokens[j + 3][0] == "("
            ):
                obj, pos = parse_object(tokens, j + 5)
                if isinstance(obj, dict) and "name" in obj:
                    defs[name] = obj
                i = pos
                continue
        i += 1
    return defs


def find_plain_objects(tokens):
    """Parse `const NAME = { ... }` object literals (shared args etc.)."""
    objs = {}
    i = 0
    while i < len(tokens):
        if (
            tokens[i][0] == "id"
            and tokens[i][1] == "const"
            and i + 3 < len(tokens)
            and tokens[i + 1][0] == "id"
            and tokens[i + 2][0] == "="
            and tokens[i + 3][0] == "{"
        ):
            name = tokens[i + 1][1]
            obj, pos = parse_object(tokens, i + 4)
            objs[name] = obj
            i = pos
            continue
        i += 1
    return objs


def parse_subcommands_map(tokens):
    """Parse `export const subCommands = { name: Ref, ... }` → ordered pairs."""
    i = 0
    while i < len(tokens):
        if tokens[i][0] == "id" and tokens[i][1] == "const":
            j = i + 1
            if (
                j < len(tokens)
                and tokens[j][0] == "id"
                and tokens[j][1] == "subCommands"
                and tokens[j + 1][0] == "="
            ):
                obj, _ = parse_object(tokens, j + 3)
                pairs = [
                    (k, v[1] if isinstance(v, tuple) and v[0] == "ref" else None)
                    for k, v in obj.items()
                ]
                return pairs
        i += 1
    return []


# ── Emit ───────────────────────────────────────────────────────────


def rust_escape(s):
    out = []
    for ch in s:
        if ch == "\\":
            out.append("\\\\")
        elif ch == '"':
            out.append('\\"')
        elif ch == "\n":
            out.append("\\n")
        elif ch == "\t":
            out.append("\\t")
        elif ch == "\r":
            out.append("\\r")
        elif ord(ch) < 0x20:
            out.append(f"\\u{{{ord(ch):x}}}")
        else:
            out.append(ch)
    return "".join(out)


def main():
    all_defs = {}  # constName -> dict
    consts = {}  # plain const NAME = {…} objects (shared args)
    for fn in FILES:
        path = os.path.join(SRC, fn)
        with open(path) as f:
            src = f.read()
        toks = lex(src)
        for name, obj in find_defs(toks).items():
            assert name not in all_defs, f"dup {name}"
            all_defs[name] = obj
        for name, obj in find_plain_objects(toks).items():
            assert name not in consts or consts[name] == obj, f"plain dup {name}"
            consts[name] = obj

    top_pairs = parse_subcommands_map(lex(open(os.path.join(SRC, "index.ts")).read()))

    # Assign ids: top-level order first (matches index.ts), then nested defs
    top_names = [ref for _, ref in top_pairs]
    nested = [n for n in all_defs if n not in top_names]
    ordered = top_names + sorted(nested)
    id_of = {name: idx for idx, name in enumerate(ordered)}

    for name in top_names:
        assert name in all_defs, f"missing def {name}"

    lines = []
    lines.append("//! Command registry — GENERATED by rust/scripts/gen_commands.py.")
    lines.append(
        "//! Do not edit by hand; regenerate from the TS reference commands/*.ts."
    )
    lines.append("")
    lines.append("use crate::help::{ArgKind, ArgSpec};")
    lines.append("")
    lines.append("/// One command definition (help surface). Subcommand indices")
    lines.append("/// reference the flat `COMMANDS` table below.")
    lines.append("#[derive(Clone, Debug)]")
    lines.append("pub struct CommandDef {")
    lines.append("    pub name: &'static str,")
    lines.append("    pub description: &'static str,")
    lines.append("    pub to_kebab: bool,")
    lines.append("    pub args: &'static [ArgSpec<'static>],")
    lines.append("    pub subcommands: &'static [u16],")
    lines.append("}")
    lines.append("")
    lines.append("pub static COMMANDS: &[CommandDef] = &[")

    for name in ordered:
        d = all_defs[name]
        args = d.get("args") or {}
        if isinstance(args, tuple) and args[0] == "ref":
            ref = args[1]
            assert ref in consts, f"{name}: shared args {ref} not found"
            args = consts[ref]
        subcmd_ids = []
        for k, v in (d.get("subCommands") or {}).items():
            ref = v[1] if isinstance(v, tuple) and v[0] == "ref" else v
            assert ref in id_of, f"{name}: unknown subcommand ref {ref}"
            subcmd_ids.append((k, id_of[ref]))
        lines.append("    CommandDef {")
        lines.append(f'        name: "{rust_escape(d.get("name", ""))}",')
        lines.append(
            f'        description: "{rust_escape(d.get("description", "") or "")}",'
        )
        lines.append(f"        to_kebab: {str(bool(d.get('toKebab'))).lower()},")
        lines.append("        args: &[")
        for k, v in args.items():
            if not isinstance(v, dict):
                continue
            kind = v.get("type", "string")
            short = v.get("short")
            desc = v.get("description") or ""
            req = v.get("required")
            lines.append("            ArgSpec {")
            lines.append(f'                name: "{rust_escape(k)}",')
            lines.append(f"                kind: ArgKind::{kind.capitalize()},")
            lines.append(
                f"                short: {('Some(' + repr(short) + ')') if short else 'None'},"
            )
            lines.append(f'                description: "{rust_escape(desc)}",')
            lines.append(f"                array: {str(bool(v.get('array'))).lower()},")
            if req is None:
                lines.append("                required: None,")
            else:
                lines.append(
                    f"                required: Some({str(bool(req)).lower()}),"
                )
            lines.append("            },")
        lines.append("        ],")
        if subcmd_ids:
            lines.append(
                "        subcommands: &["
                + ", ".join(str(i) for _, i in subcmd_ids)
                + "],"
            )
        else:
            lines.append("        subcommands: &[],")
        lines.append("    },")

    lines.append("];")
    lines.append("")

    lines.append(
        "/// Top-level subCommands map (index.ts order: name → COMMANDS index)."
    )
    lines.append("pub static TOP_SUBCOMMANDS: &[(&'static str, u16)] = &[")
    for k, ref in top_pairs:
        lines.append(f'    ("{rust_escape(k)}", {id_of[ref]}),')
    lines.append("];")
    lines.append("")
    lines.append(f"/// Total number of command definitions (incl. nested subcommands).")
    lines.append("#[allow(dead_code)]")
    lines.append(f"pub const COMMAND_COUNT: usize = {len(ordered)};")
    lines.append("")

    with open(OUT, "w") as f:
        f.write("\n".join(lines))

    print(f"wrote {OUT}: {len(ordered)} defs, {len(top_pairs)} top-level")
    for k, ref in top_pairs:
        print(f"  {k} -> {ref}")


if __name__ == "__main__":
    sys.exit(main())
