import { canonicalizeBaseType, cloneBoxes, formatValueForType, normalizeSpecialFloatLiteral, parseDoubleValueWithSign, parseType, randAddr, stripAllComments, typeInfo, } from "./shared-core-utils.js";
import { createParserTools, } from "./shared-core-parser.js";
export function createSimpleSimulator() {
    const requireSourceValue = true;
    const isEvalError = (result) => !!result.error;
    const isScalarError = (result) => !!result.error;
    function decayArrayValue(result) {
        if (isEvalError(result))
            return result;
        if (!result.isArray)
            return result;
        const arrayShape = normalizeArrayDims(result.arrayShape);
        const decayedPointeeArrayDims = arrayShape.length > 1
            ? arrayShape.slice(1)
            : normalizeArrayDims(result.pointeeArrayDims);
        return {
            kind: "rvalue",
            base: result.base,
            depth: result.depth,
            value: result.value,
            address: "",
            label: result.label,
            nanSign: result.nanSign,
            pointeeArrayDims: decayedPointeeArrayDims,
            pointeeInnerDepth: normalizePointeeInnerDepth(result.pointeeInnerDepth, result.depth, decayedPointeeArrayDims),
        };
    }
    function hasDeclaredPrefix(prefix, names) {
        if (!prefix || !names || !names.size)
            return false;
        for (const name of names) {
            if (name.startsWith(prefix))
                return true;
        }
        return false;
    }
    function arrayElementName(name, indices) {
        return `${name}${indices.map((index) => `[${index}]`).join("")}`;
    }
    function parseArrayElementName(name) {
        const match = /^([A-Za-z_][A-Za-z0-9_]*)((?:\[\d+\])+)$/.exec(String(name || ""));
        if (!match)
            return null;
        const baseName = match[1] || "";
        const suffix = match[2] || "";
        if (!baseName || !suffix)
            return null;
        const indices = [];
        const rx = /\[(\d+)\]/g;
        let m;
        while ((m = rx.exec(suffix)) != null) {
            const value = Number(m[1]);
            if (!Number.isFinite(value) || value < 0)
                return null;
            indices.push(value);
        }
        if (!indices.length)
            return null;
        return { baseName, indices };
    }
    function forEachArrayIndex(shape, fn) {
        const dims = shape.map((d) => Math.max(0, Math.floor(Number(d))));
        if (!dims.length || dims.some((d) => d <= 0))
            return;
        let linear = 0;
        const indices = new Array(dims.length).fill(0);
        const recur = (depth) => {
            if (depth >= dims.length) {
                fn(indices.slice(), linear++);
                return;
            }
            for (let i = 0; i < dims[depth]; i++) {
                indices[depth] = i;
                recur(depth + 1);
            }
        };
        recur(0);
    }
    function makeArrayDeclaredNames(name, shape) {
        const out = [name];
        forEachArrayIndex(shape, (indices) => {
            out.push(arrayElementName(name, indices));
        });
        return out;
    }
    function normalizeArrayDims(dims) {
        if (!Array.isArray(dims))
            return [];
        const out = [];
        for (const dim of dims) {
            const value = Math.floor(Number(dim));
            if (!Number.isFinite(value) || value <= 0)
                return [];
            out.push(value);
        }
        return out;
    }
    function sameArrayDims(left, right) {
        if (left.length !== right.length)
            return false;
        for (let i = 0; i < left.length; i++) {
            if (left[i] !== right[i])
                return false;
        }
        return true;
    }
    function normalizePointeeInnerDepth(value, depth, pointeeArrayDims) {
        if (!Array.isArray(pointeeArrayDims) || pointeeArrayDims.length === 0)
            return 0;
        const raw = Math.floor(Number(value));
        const normalized = Number.isFinite(raw) ? raw : 0;
        const maxInner = Math.max(0, Math.floor(Number(depth)) - 1);
        return Math.max(0, Math.min(normalized, maxInner));
    }
    function samePointerPointeeType(leftDepth, leftPointeeArrayDims, leftPointeeInnerDepth, rightDepth, rightPointeeArrayDims, rightPointeeInnerDepth) {
        if (!sameArrayDims(leftPointeeArrayDims, rightPointeeArrayDims))
            return false;
        if (!leftPointeeArrayDims.length)
            return true;
        return (normalizePointeeInnerDepth(leftPointeeInnerDepth, leftDepth, leftPointeeArrayDims) ===
            normalizePointeeInnerDepth(rightPointeeInnerDepth, rightDepth, rightPointeeArrayDims));
    }
    function arrayElementCount(shape) {
        let count = 1;
        for (const dim of shape) {
            count *= dim;
        }
        return count;
    }
    function arrayLinearIndex(indices, shape) {
        if (indices.length !== shape.length)
            return null;
        let linear = 0;
        let stride = 1;
        for (let i = shape.length - 1; i >= 0; i--) {
            const idx = indices[i];
            const dim = shape[i];
            if (idx < 0 || idx >= dim)
                return null;
            linear += idx * stride;
            stride *= dim;
        }
        return linear;
    }
    function isBraceToken(tok) {
        return tok.type === "sym" && (tok.value === "{" || tok.value === "}");
    }
    function addDeclaredNames(scopes, declared, names) {
        for (const name of names)
            declared.add(name);
        const current = scopes[scopes.length - 1];
        if (current) {
            for (const name of names)
                current.add(name);
        }
    }
    function popScope(scopes, declared, state) {
        if (scopes.length <= 1) {
            return { state, error: "Unexpected }." };
        }
        const frame = scopes.pop();
        if (!frame || frame.size === 0)
            return { state };
        const namesToRemove = new Set(frame);
        const nextState = state.filter((box) => !namesToRemove.has(box.name));
        frame.forEach((name) => declared.delete(name));
        return { state: nextState };
    }
    function resolveDeclType(stars, baseType = "int") {
        if (!Number.isFinite(stars) || stars < 0)
            return null;
        const base = canonicalizeBaseType(baseType);
        if (!base)
            return null;
        if (stars === 0)
            return base;
        return `${base}${"*".repeat(stars)}`;
    }
    function makePointerType(depth, base = "int", pointeeArrayDims = [], pointeeInnerDepth) {
        if (!Number.isFinite(depth) || depth < 0)
            return null;
        const canonicalBase = canonicalizeBaseType(base);
        if (!canonicalBase)
            return null;
        const dims = normalizeArrayDims(pointeeArrayDims)
            .map((d) => `[${d}]`)
            .join("");
        if (depth === 0)
            return `${canonicalBase}${dims}`;
        if (Array.isArray(pointeeArrayDims) && pointeeArrayDims.length > 0) {
            const rawInner = Math.floor(Number(pointeeInnerDepth));
            const innerDepth = Math.max(0, Math.min(Number.isFinite(rawInner) ? rawInner : 0, Math.max(0, Math.floor(depth))));
            const outerDepth = Math.max(0, depth - innerDepth);
            const inner = innerDepth > 0 ? ` ${"*".repeat(innerDepth)}` : "";
            if (outerDepth === 0) {
                return `${canonicalBase}${inner}${dims}`;
            }
            return `${canonicalBase}${inner} (${"*".repeat(outerDepth)})${dims}`;
        }
        return `${canonicalBase}${"*".repeat(depth)}`;
    }
    const INTEGER_TYPE_META = {
        bool: { bits: 1, signed: false, rank: 1 },
        char: { bits: 8, signed: true, rank: 2 },
        "signed char": { bits: 8, signed: true, rank: 2 },
        "unsigned char": { bits: 8, signed: false, rank: 2 },
        short: { bits: 16, signed: true, rank: 3 },
        "unsigned short": { bits: 16, signed: false, rank: 3 },
        int: { bits: 32, signed: true, rank: 4 },
        "unsigned int": { bits: 32, signed: false, rank: 4 },
        long: { bits: 64, signed: true, rank: 5 },
        "unsigned long": { bits: 64, signed: false, rank: 5 },
        "long long": { bits: 64, signed: true, rank: 6 },
        "unsigned long long": { bits: 64, signed: false, rank: 6 },
    };
    function integerMetaForBase(base) {
        return INTEGER_TYPE_META[base] || null;
    }
    function isFloatingBase(base) {
        return base === "float" || base === "double";
    }
    function isIntegerBase(base) {
        return !!integerMetaForBase(base);
    }
    function integerRangeForBase(base) {
        const meta = integerMetaForBase(base);
        if (!meta)
            return null;
        if (!meta.signed) {
            return {
                min: 0n,
                max: (1n << BigInt(meta.bits)) - 1n,
                bits: meta.bits,
                signed: false,
            };
        }
        const max = (1n << BigInt(meta.bits - 1)) - 1n;
        const min = -(1n << BigInt(meta.bits - 1));
        return { min, max, bits: meta.bits, signed: true };
    }
    function stripIntegerSuffix(value) {
        const raw = value.trim();
        if (!raw)
            return raw;
        const parsed = parseIntegerSuffixInfo(raw);
        if (!parsed)
            return raw;
        return parsed.core;
    }
    function parseIntegerSuffixInfo(value) {
        const raw = String(value || "").trim();
        if (!raw)
            return null;
        let idx = raw.length;
        while (idx > 0 && /[uUlL]/.test(raw[idx - 1]))
            idx--;
        const core = raw.slice(0, idx);
        const suffix = raw.slice(idx).toLowerCase();
        if (!suffix) {
            return { core, unsigned: false, longCount: 0 };
        }
        if (suffix !== "u" &&
            suffix !== "l" &&
            suffix !== "ll" &&
            suffix !== "ul" &&
            suffix !== "lu" &&
            suffix !== "ull" &&
            suffix !== "llu") {
            return null;
        }
        const unsigned = suffix.includes("u");
        const longCount = suffix.includes("ll")
            ? 2
            : suffix.includes("l")
                ? 1
                : 0;
        return { core, unsigned, longCount };
    }
    function parseIntegerLiteral(value) {
        const raw = stripIntegerSuffix(value);
        if (!raw)
            return null;
        if (raw.startsWith("0x") || raw.startsWith("0X")) {
            const digits = raw.slice(2);
            if (!digits || !/^[0-9a-fA-F]+$/.test(digits))
                return null;
            try {
                return BigInt(`0x${digits}`);
            }
            catch {
                return null;
            }
        }
        if (raw.length > 1 && raw.startsWith("0")) {
            if (!/^0[0-7]+$/.test(raw))
                return null;
            try {
                return BigInt(`0o${raw.slice(1)}`);
            }
            catch {
                return null;
            }
        }
        try {
            return BigInt(raw);
        }
        catch {
            return null;
        }
    }
    function isSpecialFloatLiteral(value) {
        return !!normalizeSpecialFloatLiteral(value);
    }
    function isDecimalLiteral(value) {
        return /[.eE]/.test(String(value));
    }
    function isNonDecimalIntegerLiteral(value) {
        const core = stripIntegerSuffix(value);
        return /^0[xX]/.test(core) || /^0[0-7]/.test(core);
    }
    function fitsIntegerLiteralInBase(value, base) {
        const range = integerRangeForBase(base);
        if (!range)
            return false;
        return value >= range.min && value <= range.max;
    }
    function inferIntegerLiteralBase(value) {
        const info = parseIntegerSuffixInfo(value);
        if (!info)
            return null;
        const literalValue = parseIntegerLiteral(value);
        if (literalValue == null)
            return null;
        const nonDecimal = isNonDecimalIntegerLiteral(value);
        const candidates = [];
        if (info.unsigned) {
            if (info.longCount === 0)
                candidates.push("unsigned int", "unsigned long", "unsigned long long");
            else if (info.longCount === 1)
                candidates.push("unsigned long", "unsigned long long");
            else
                candidates.push("unsigned long long");
        }
        else if (info.longCount === 0) {
            if (nonDecimal) {
                candidates.push("int", "unsigned int", "long", "unsigned long", "long long", "unsigned long long");
            }
            else {
                candidates.push("int", "long", "long long");
            }
        }
        else if (info.longCount === 1) {
            if (nonDecimal) {
                candidates.push("long", "unsigned long", "long long", "unsigned long long");
            }
            else {
                candidates.push("long", "long long");
            }
        }
        else {
            if (nonDecimal) {
                candidates.push("long long", "unsigned long long");
            }
            else {
                candidates.push("long long");
            }
        }
        for (const candidate of candidates) {
            if (fitsIntegerLiteralInBase(literalValue, candidate))
                return candidate;
        }
        return null;
    }
    function parseNumericLiteralValue(value) {
        const trimmed = String(value || "").trim();
        if (!trimmed) {
            return { error: "That expression is not valid here.", kind: "compile" };
        }
        if (isSpecialFloatLiteral(trimmed)) {
            const parsed = parseDoubleValueWithSign(trimmed);
            if (!parsed) {
                return { error: "That number is too large to represent.", kind: "compile" };
            }
            return { base: "double", value: parsed.value, nanSign: parsed.nanSign };
        }
        if (isDecimalLiteral(trimmed)) {
            const lower = trimmed.toLowerCase();
            if (lower.endsWith("l")) {
                return { error: "long double is not supported.", kind: "compile" };
            }
            const isFloat = lower.endsWith("f");
            const core = isFloat ? trimmed.slice(0, -1) : trimmed;
            const parsed = parseDoubleValueWithSign(core);
            if (!parsed || !Number.isFinite(parsed.value) && !Number.isNaN(parsed.value)) {
                return { error: "That number is too large to represent.", kind: "compile" };
            }
            const base = isFloat ? "float" : "double";
            const castValue = base === "float" ? Math.fround(parsed.value) : parsed.value;
            return { base, value: castValue, nanSign: parsed.nanSign };
        }
        const literalBase = inferIntegerLiteralBase(trimmed);
        if (!literalBase) {
            return { error: "That number is too large to represent.", kind: "compile" };
        }
        const parsedInt = parseIntegerLiteral(trimmed);
        if (parsedInt == null) {
            return { error: "That number is too large to represent.", kind: "compile" };
        }
        const wrapped = normalizeIntegerForBase(parsedInt, literalBase);
        if (wrapped == null) {
            return { error: "That number is too large to represent.", kind: "compile" };
        }
        return { base: literalBase, value: wrapped };
    }
    function integerOverflowError(base) {
        const article = /^[aeiou]/i.test(base) ? "an" : "a";
        return {
            error: `That calculation overflows ${article} ${base}.`,
            kind: "ub",
        };
    }
    function checkIntegerRange(value, base) {
        const range = integerRangeForBase(base);
        if (!range)
            return null;
        if (value < range.min || value > range.max)
            return integerOverflowError(base);
        return null;
    }
    function bitWidthForBase(base) {
        const meta = integerMetaForBase(base);
        return meta ? meta.bits : null;
    }
    function unsignedVariantForBase(base) {
        if (base === "char" || base === "signed char" || base === "unsigned char")
            return "unsigned char";
        if (base === "short" || base === "unsigned short")
            return "unsigned short";
        if (base === "int" || base === "unsigned int")
            return "unsigned int";
        if (base === "long" || base === "unsigned long")
            return "unsigned long";
        if (base === "long long" || base === "unsigned long long")
            return "unsigned long long";
        if (base === "bool")
            return "bool";
        return null;
    }
    function signedVariantForBase(base) {
        if (base === "char" || base === "signed char" || base === "unsigned char")
            return "signed char";
        if (base === "short" || base === "unsigned short")
            return "short";
        if (base === "int" || base === "unsigned int")
            return "int";
        if (base === "long" || base === "unsigned long")
            return "long";
        if (base === "long long" || base === "unsigned long long")
            return "long long";
        if (base === "bool")
            return "bool";
        return null;
    }
    function integerPromotionBase(base) {
        const meta = integerMetaForBase(base);
        if (!meta)
            return "int";
        if (meta.rank < INTEGER_TYPE_META.int.rank)
            return "int";
        return base;
    }
    function canSignedRepresentUnsigned(signedBase, unsignedBase) {
        const signedRange = integerRangeForBase(signedBase);
        const unsignedRange = integerRangeForBase(unsignedBase);
        if (!signedRange || !unsignedRange)
            return false;
        if (!signedRange.signed || unsignedRange.signed)
            return false;
        return signedRange.max >= unsignedRange.max;
    }
    function usualIntegerBase(leftBase, rightBase) {
        const leftPromoted = integerPromotionBase(leftBase);
        const rightPromoted = integerPromotionBase(rightBase);
        const leftMeta = integerMetaForBase(leftPromoted);
        const rightMeta = integerMetaForBase(rightPromoted);
        if (!leftMeta || !rightMeta)
            return "int";
        if (leftPromoted === rightPromoted)
            return leftPromoted;
        if (leftMeta.signed === rightMeta.signed) {
            return leftMeta.rank >= rightMeta.rank ? leftPromoted : rightPromoted;
        }
        const signedBase = leftMeta.signed ? leftPromoted : rightPromoted;
        const unsignedBase = leftMeta.signed ? rightPromoted : leftPromoted;
        const signedMeta = integerMetaForBase(signedBase);
        const unsignedMeta = integerMetaForBase(unsignedBase);
        if (!signedMeta || !unsignedMeta)
            return "int";
        if (unsignedMeta.rank >= signedMeta.rank) {
            return unsignedBase;
        }
        if (canSignedRepresentUnsigned(signedBase, unsignedBase)) {
            return signedBase;
        }
        return unsignedVariantForBase(signedBase) || unsignedBase;
    }
    function wrapIntegerToBase(value, base) {
        const range = integerRangeForBase(base);
        if (!range)
            return null;
        const width = range.bits;
        const modulo = 1n << BigInt(width);
        let wrapped = value % modulo;
        if (wrapped < 0n)
            wrapped += modulo;
        if (range.signed) {
            const signBit = 1n << BigInt(width - 1);
            if (wrapped >= signBit)
                wrapped -= modulo;
        }
        return wrapped;
    }
    function normalizeIntegerForBase(value, base) {
        return wrapIntegerToBase(value, base);
    }
    const parser = createParserTools({
        evaluateArrayLengthExpr: (expr) => {
            const evaluated = evaluateExpression(expr, [], {
                targetType: "int",
                requireValue: true,
            });
            if (isScalarError(evaluated))
                return null;
            if (!isIntegerBase(evaluated.base))
                return null;
            const raw = evaluated.value;
            if (typeof raw !== "bigint")
                return null;
            if (raw <= 0n)
                return null;
            if (raw > BigInt(Number.MAX_SAFE_INTEGER))
                return null;
            return Number(raw);
        },
    });
    const tokenizeProgram = parser.tokenizeProgram;
    const parseExpressionTokens = parser.parseExpressionTokens;
    const parseDeclHead = parser.parseDeclHead;
    const parseIfHeaderTokens = parser.parseIfHeaderTokens;
    const parseWhileHeaderTokens = parser.parseWhileHeaderTokens;
    const parseStatementTokens = parser.parseStatementTokens;
    const isStatementPrefix = parser.isStatementPrefix;
    const controlHeaderEndIndex = parser.controlHeaderEndIndex;
    const splitStatements = parser.splitStatements;
    function coerceScalarResult(result, requireValue) {
        if (!result)
            return {
                error: "That expression is not valid here.",
                kind: "compile",
            };
        if (result.error)
            return result;
        const source = decayArrayValue(result);
        if (isEvalError(source))
            return source;
        if (!Number.isFinite(source.depth) || source.depth !== 0)
            return {
                error: "That expression must be a scalar value.",
                kind: "compile",
            };
        const raw = source.value;
        if (source.kind === "lvalue") {
            if (requireValue && String(raw ?? "") === "") {
                const label = source.label || "That value";
                return { error: `${label} doesn't have a value yet.`, kind: "ub" };
            }
        }
        const base = source.base || "int";
        if (isFloatingBase(base)) {
            const rawValue = raw ?? "";
            const parsed = parseDoubleValueWithSign(rawValue);
            if (!parsed) {
                const label = source.label || "That value";
                return { error: `${label} isn't a number.`, kind: "compile" };
            }
            const nanSign = "nanSign" in source && source.nanSign !== undefined
                ? source.nanSign
                : parsed.nanSign;
            return { value: parsed.value, base, nanSign };
        }
        try {
            const value = typeof raw === "bigint" ? raw : BigInt(String(raw));
            return { value, base };
        }
        catch {
            const label = source.label || "That value";
            return { error: `${label} isn't a number.`, kind: "compile" };
        }
    }
    function evaluateExpressionRaw(expr, state, opts = {}) {
        const { targetType = "int", requireValue = requireSourceValue, onSideEffect, } = opts;
        const by = Object.fromEntries(state.map((b) => [b.name, b]));
        const arraysByName = (() => {
            const grouped = new Map();
            for (const box of state) {
                let baseName = "";
                let indices = [];
                const metadataShape = normalizeArrayDims(box.arrayShape);
                if (box.arrayRoot && Array.isArray(box.arrayIndices) && box.arrayIndices.length > 0) {
                    baseName = String(box.arrayRoot);
                    indices = box.arrayIndices
                        .map((raw) => Math.floor(Number(raw)))
                        .filter((value) => Number.isFinite(value) && value >= 0);
                }
                else {
                    const parsedName = parseArrayElementName(box.name);
                    if (!parsedName)
                        continue;
                    baseName = parsedName.baseName;
                    indices = parsedName.indices;
                }
                if (!baseName || !indices.length)
                    continue;
                const linearIndex = arrayLinearIndex(indices, metadataShape) ??
                    (indices.length === 1 ? indices[0] : null);
                if (!Number.isFinite(linearIndex) || linearIndex < 0)
                    continue;
                const parsedType = parseType(box.type);
                if (!parsedType.base || !Number.isFinite(parsedType.depth))
                    continue;
                const elementPointeeArrayDims = normalizeArrayDims(box.pointeeArrayDims);
                const elementPointeeInnerDepth = normalizePointeeInnerDepth(box.pointeeInnerDepth ?? parsedType.pointeeInnerDepth, parsedType.depth, elementPointeeArrayDims);
                const existing = grouped.get(baseName);
                if (!existing) {
                    grouped.set(baseName, {
                        elementType: box.type,
                        elementBase: parsedType.base,
                        elementDepth: parsedType.depth,
                        elementPointeeArrayDims,
                        elementPointeeInnerDepth,
                        shape: metadataShape,
                        byIndex: new Map([[linearIndex, box]]),
                    });
                    continue;
                }
                if (existing.elementType !== box.type ||
                    existing.elementBase !== parsedType.base ||
                    existing.elementDepth !== parsedType.depth ||
                    !sameArrayDims(existing.elementPointeeArrayDims, elementPointeeArrayDims) ||
                    existing.elementPointeeInnerDepth !== elementPointeeInnerDepth) {
                    continue;
                }
                if (!existing.shape.length && metadataShape.length) {
                    existing.shape = metadataShape;
                }
                else if (existing.shape.length &&
                    metadataShape.length &&
                    !sameArrayDims(existing.shape, metadataShape)) {
                    continue;
                }
                existing.byIndex.set(linearIndex, box);
            }
            const out = new Map();
            for (const [name, entry] of grouped.entries()) {
                const first = entry.byIndex.get(0);
                if (!first || !String(first.address ?? "").trim())
                    continue;
                let shape = entry.shape.slice();
                let length = 0;
                if (shape.length) {
                    length = arrayElementCount(shape);
                }
                else {
                    while (entry.byIndex.has(length))
                        length++;
                    if (length > 0)
                        shape = [length];
                }
                if (length <= 0)
                    continue;
                out.set(name, {
                    name,
                    elementType: entry.elementType,
                    elementBase: entry.elementBase,
                    elementDepth: entry.elementDepth,
                    elementPointeeArrayDims: entry.elementPointeeArrayDims.slice(),
                    elementPointeeInnerDepth: entry.elementPointeeInnerDepth,
                    shape: shape.slice(),
                    length,
                    baseAddress: String(first.address ?? "").trim(),
                    byIndex: entry.byIndex,
                });
            }
            return out;
        })();
        const toNumber = (value) => typeof value === "bigint" ? Number(value) : value;
        const toUnsignedBits = (value, base) => {
            const width = bitWidthForBase(base);
            if (!width)
                return value;
            const modulo = 1n << BigInt(width);
            let next = value % modulo;
            if (next < 0n)
                next += modulo;
            return next;
        };
        const pointerStepSize = (base, depth, pointeeArrayDims = [], pointeeInnerDepth) => {
            const pointeeDepth = Math.max(0, depth - 1);
            const pointeeType = makePointerType(pointeeDepth, base, pointeeArrayDims, pointeeInnerDepth) || base;
            const info = typeInfo(pointeeType);
            const size = Number.isFinite(info.size) && info.size > 0 ? info.size : 1;
            return BigInt(size);
        };
        function makeLvalue(box, label) {
            const { base, depth, pointeeArrayDims: parsedPointeeArrayDims, pointeeInnerDepth: parsedPointeeInnerDepth, } = parseType(box.type);
            if (!base || !Number.isFinite(depth)) {
                return {
                    error: "That expression is not valid here.",
                    kind: "compile",
                };
            }
            const pointeeArrayDims = (() => {
                const fromBox = normalizeArrayDims(box.pointeeArrayDims);
                if (fromBox.length)
                    return fromBox;
                return normalizeArrayDims(parsedPointeeArrayDims);
            })();
            const pointeeInnerDepth = normalizePointeeInnerDepth(box.pointeeInnerDepth ?? parsedPointeeInnerDepth, depth, pointeeArrayDims);
            let nanSign;
            if (isFloatingBase(base)) {
                const parsed = parseDoubleValueWithSign(box.value);
                nanSign = parsed?.nanSign;
            }
            return {
                kind: "lvalue",
                base,
                depth,
                value: box.value,
                address: box.address ?? "",
                label: label || box.name,
                nanSign,
                pointeeArrayDims,
                pointeeInnerDepth,
            };
        }
        function makeArrayLvalue(info, label) {
            let ptr;
            try {
                ptr = BigInt(String(info.baseAddress || "0").trim() || "0");
            }
            catch {
                return {
                    error: "That expression is not valid here.",
                    kind: "compile",
                };
            }
            return {
                kind: "lvalue",
                base: info.elementBase,
                depth: info.elementDepth + 1,
                value: ptr,
                address: String(info.baseAddress || ""),
                label,
                isArray: true,
                arrayShape: info.shape.slice(),
                pointeeArrayDims: info.shape.length > 1
                    ? info.shape.slice(1)
                    : info.elementPointeeArrayDims.slice(),
                pointeeInnerDepth: info.shape.length > 1
                    ? Math.max(0, info.elementDepth)
                    : normalizePointeeInnerDepth(info.elementPointeeInnerDepth, info.elementDepth + 1, info.elementPointeeArrayDims),
            };
        }
        function makeCompileError(message) {
            return { error: message, kind: "compile" };
        }
        function makeUbError(message) {
            return { error: message, kind: "ub" };
        }
        function makeRvalue(value, base, depth = 0, label = "", nanSign, pointeeArrayDims = [], pointeeInnerDepth) {
            const normalizedPointeeArrayDims = normalizeArrayDims(pointeeArrayDims);
            return {
                kind: "rvalue",
                base,
                depth,
                value,
                address: "",
                label,
                nanSign,
                pointeeArrayDims: normalizedPointeeArrayDims,
                pointeeInnerDepth: normalizePointeeInnerDepth(pointeeInnerDepth, depth, normalizedPointeeArrayDims),
            };
        }
        function runtimeFromEval(evaluated, mustHaveValue = requireValue) {
            if (isEvalError(evaluated))
                return evaluated;
            const decayed = decayArrayValue(evaluated);
            if (isEvalError(decayed))
                return decayed;
            const raw = decayed.value;
            if (mustHaveValue && String(raw ?? "") === "") {
                const label = decayed.label || "That value";
                return makeUbError(`${label} doesn't have a value yet.`);
            }
            if (decayed.depth > 0) {
                const trimmed = String(raw ?? "").trim();
                if (!trimmed) {
                    const label = decayed.label || "That pointer";
                    return makeUbError(`${label} doesn't have a value yet.`);
                }
                try {
                    return {
                        base: decayed.base,
                        depth: decayed.depth,
                        value: BigInt(trimmed),
                        nanSign: decayed.nanSign,
                        label: decayed.label || "",
                        address: decayed.address || "",
                        kind: decayed.kind,
                        pointeeArrayDims: normalizeArrayDims(decayed.pointeeArrayDims),
                        pointeeInnerDepth: normalizePointeeInnerDepth(decayed.pointeeInnerDepth, decayed.depth, normalizeArrayDims(decayed.pointeeArrayDims)),
                    };
                }
                catch {
                    return makeCompileError("Pointer values must be integer addresses.");
                }
            }
            if (isFloatingBase(decayed.base)) {
                const parsed = parseDoubleValueWithSign(raw);
                if (!parsed) {
                    const label = decayed.label || "That value";
                    return makeCompileError(`${label} isn't a number.`);
                }
                return {
                    base: decayed.base,
                    depth: 0,
                    value: parsed.value,
                    nanSign: decayed.nanSign !== undefined ? decayed.nanSign : parsed.nanSign,
                    label: decayed.label || "",
                    address: decayed.address || "",
                    kind: decayed.kind,
                    pointeeArrayDims: [],
                    pointeeInnerDepth: 0,
                };
            }
            if (!isIntegerBase(decayed.base)) {
                return makeCompileError("That expression is not valid here.");
            }
            try {
                const parsedInt = typeof raw === "bigint" ? raw : BigInt(String(raw ?? "").trim() || "0");
                const normalized = normalizeIntegerForBase(parsedInt, decayed.base);
                if (normalized == null) {
                    return makeCompileError("That expression is not valid here.");
                }
                return {
                    base: decayed.base,
                    depth: 0,
                    value: normalized,
                    nanSign: decayed.nanSign,
                    label: decayed.label || "",
                    address: decayed.address || "",
                    kind: decayed.kind,
                    pointeeArrayDims: [],
                    pointeeInnerDepth: 0,
                };
            }
            catch {
                const label = decayed.label || "That value";
                return makeCompileError(`${label} isn't a number.`);
            }
        }
        function runtimeTruthy(value) {
            if (value.depth > 0)
                return value.value !== 0n;
            if (isFloatingBase(value.base)) {
                const num = value.value;
                return num !== 0;
            }
            return value.value !== 0n;
        }
        function resolveTargetBox(target) {
            if (isEvalError(target))
                return null;
            if (target.kind !== "lvalue")
                return null;
            return (state.find((box) => String(box.address ?? "") === String(target.address ?? "")) ||
                null);
        }
        function pointerCanReferenceTarget(pointer, target) {
            const pointerBase = pointer.base || "int";
            const pointerDepth = Number.isFinite(pointer.depth) ? Math.floor(pointer.depth) : 0;
            if (pointerDepth < 1)
                return false;
            const pointerPointeeArrayDims = normalizeArrayDims(pointer.pointeeArrayDims);
            const pointerPointeeInnerDepth = normalizePointeeInnerDepth(pointer.pointeeInnerDepth, pointerDepth, pointerPointeeArrayDims);
            const pointerOuterDepth = Math.max(0, pointerDepth - pointerPointeeInnerDepth);
            const parsedTarget = parseType(target.type || "int");
            if (!parsedTarget.base || !Number.isFinite(parsedTarget.depth))
                return false;
            if (parsedTarget.base !== pointerBase)
                return false;
            const targetDepth = Math.floor(parsedTarget.depth);
            const targetPointeeArrayDims = (() => {
                const fromBox = normalizeArrayDims(target.pointeeArrayDims);
                if (fromBox.length)
                    return fromBox;
                return normalizeArrayDims(parsedTarget.pointeeArrayDims);
            })();
            const targetPointeeInnerDepth = normalizePointeeInnerDepth(target.pointeeInnerDepth ?? parsedTarget.pointeeInnerDepth, targetDepth, targetPointeeArrayDims);
            if (!pointerPointeeArrayDims.length) {
                return targetDepth === pointerDepth - 1 && targetPointeeArrayDims.length === 0;
            }
            if (pointerOuterDepth === 1) {
                return (targetDepth === pointerPointeeInnerDepth &&
                    targetPointeeArrayDims.length === 0);
            }
            return (targetDepth === pointerDepth - 1 &&
                samePointerPointeeType(targetDepth, targetPointeeArrayDims, targetPointeeInnerDepth, pointerDepth - 1, pointerPointeeArrayDims, pointerPointeeInnerDepth));
        }
        function assignIntoTarget(target, source) {
            if (isEvalError(target))
                return target;
            if (target.kind !== "lvalue") {
                return makeCompileError("That assignment is not valid here.");
            }
            if (target.isArray) {
                return makeCompileError("That assignment is not valid here.");
            }
            const targetType = makePointerType(Number.isFinite(target.depth) ? target.depth : 0, target.base || "int", normalizeArrayDims(target.pointeeArrayDims), target.pointeeInnerDepth) || "int";
            const converted = convertAssignmentValue(source, targetType, requireValue);
            if ("kind" in converted && converted.kind === "type-mismatch") {
                return makeCompileError("That assignment is not valid here.");
            }
            if ("error" in converted)
                return converted;
            const box = resolveTargetBox(target);
            if (!box)
                return makeCompileError("That assignment is not valid here.");
            box.value = converted.value;
            onSideEffect?.();
            if (target.depth > 0) {
                const nextAddress = String(box.value ?? "").trim();
                try {
                    return makeRvalue(BigInt(nextAddress || "0"), target.base, target.depth, target.label, converted.nanSign, normalizeArrayDims(target.pointeeArrayDims), target.pointeeInnerDepth);
                }
                catch {
                    return makeCompileError("That assignment is not valid here.");
                }
            }
            if (isFloatingBase(target.base)) {
                const parsed = parseDoubleValueWithSign(box.value);
                if (!parsed)
                    return makeCompileError("That assignment is not valid here.");
                return makeRvalue(parsed.value, target.base, 0, target.label, converted.nanSign ?? parsed.nanSign);
            }
            try {
                const parsed = BigInt(String(box.value ?? "").trim() || "0");
                const normalized = normalizeIntegerForBase(parsed, target.base);
                if (normalized == null)
                    return makeCompileError("That assignment is not valid here.");
                return makeRvalue(normalized, target.base, 0, target.label);
            }
            catch {
                return makeCompileError("That assignment is not valid here.");
            }
        }
        function evaluateBinaryResolved(op, leftEval, rightEval) {
            const left = runtimeFromEval(leftEval, true);
            if (isEvalError(left))
                return left;
            const right = runtimeFromEval(rightEval, true);
            if (isEvalError(right))
                return right;
            if (op === "==" || op === "!=") {
                if (left.depth > 0 || right.depth > 0) {
                    let result = false;
                    if (left.depth > 0 && right.depth > 0) {
                        if (left.base !== right.base ||
                            left.depth !== right.depth ||
                            !samePointerPointeeType(left.depth, left.pointeeArrayDims, left.pointeeInnerDepth, right.depth, right.pointeeArrayDims, right.pointeeInnerDepth)) {
                            return makeCompileError("That expression is not valid here.");
                        }
                        result = left.value === right.value;
                    }
                    else {
                        const pointerValue = left.depth > 0 ? left.value : right.value;
                        const scalar = left.depth > 0 ? right : left;
                        if (scalar.depth !== 0 || !isIntegerBase(scalar.base)) {
                            return makeCompileError("That expression is not valid here.");
                        }
                        result = pointerValue === scalar.value;
                    }
                    return makeRvalue(result === (op === "==") ? 1n : 0n, "int");
                }
            }
            if (op === "&&" || op === "||") {
                const leftTruthy = runtimeTruthy(left);
                const rightTruthy = runtimeTruthy(right);
                const value = op === "&&" ? leftTruthy && rightTruthy : leftTruthy || rightTruthy;
                return makeRvalue(value ? 1n : 0n, "int");
            }
            if (op === "+" || op === "-") {
                if (left.depth > 0 || right.depth > 0) {
                    if (op === "+" && left.depth > 0 && right.depth > 0) {
                        return makeCompileError("Pointer addition is not valid.");
                    }
                    if (op === "-" &&
                        left.depth > 0 &&
                        right.depth > 0) {
                        if (left.base !== right.base || left.depth !== right.depth) {
                            return makeCompileError("That expression is not valid here.");
                        }
                        if (!samePointerPointeeType(left.depth, left.pointeeArrayDims, left.pointeeInnerDepth, right.depth, right.pointeeArrayDims, right.pointeeInnerDepth)) {
                            return makeCompileError("That expression is not valid here.");
                        }
                        const step = pointerStepSize(left.base, left.depth, left.pointeeArrayDims, left.pointeeInnerDepth);
                        if (step === 0n)
                            return makeCompileError("That expression is not valid here.");
                        const diff = (left.value - right.value) / step;
                        const normalized = normalizeIntegerForBase(diff, "long");
                        if (normalized == null)
                            return makeCompileError("That expression is not valid here.");
                        return makeRvalue(normalized, "long");
                    }
                    const pointer = left.depth > 0 ? left : right;
                    const scalar = left.depth > 0 ? right : left;
                    if (scalar.depth !== 0 || !isIntegerBase(scalar.base)) {
                        return makeCompileError("That expression is not valid here.");
                    }
                    const step = pointerStepSize(pointer.base, pointer.depth, pointer.pointeeArrayDims, pointer.pointeeInnerDepth);
                    const delta = scalar.value;
                    const signedDelta = left.depth > 0
                        ? op === "+" ? delta : -delta
                        : delta;
                    const nextAddress = pointer.value + signedDelta * step;
                    return makeRvalue(nextAddress, pointer.base, pointer.depth, "", undefined, pointer.pointeeArrayDims, pointer.pointeeInnerDepth);
                }
            }
            if ((op === "<" || op === "<=" || op === ">" || op === ">=") &&
                (left.depth > 0 || right.depth > 0)) {
                if (left.depth <= 0 || right.depth <= 0) {
                    return makeCompileError("That expression is not valid here.");
                }
                if (left.base !== right.base ||
                    left.depth !== right.depth ||
                    !samePointerPointeeType(left.depth, left.pointeeArrayDims, left.pointeeInnerDepth, right.depth, right.pointeeArrayDims, right.pointeeInnerDepth)) {
                    return makeCompileError("That expression is not valid here.");
                }
                const lhs = left.value;
                const rhs = right.value;
                const ok = op === "<"
                    ? lhs < rhs
                    : op === "<="
                        ? lhs <= rhs
                        : op === ">"
                            ? lhs > rhs
                            : lhs >= rhs;
                return makeRvalue(ok ? 1n : 0n, "int");
            }
            if (left.depth > 0 || right.depth > 0) {
                return makeCompileError("That expression is not valid here.");
            }
            const useFloat = isFloatingBase(left.base) || isFloatingBase(right.base);
            if (useFloat) {
                if (op === "&" ||
                    op === "|" ||
                    op === "^" ||
                    op === "<<" ||
                    op === ">>" ||
                    op === "%") {
                    return makeCompileError("Bitwise operators require integer values.");
                }
                const resultBase = left.base === "double" || right.base === "double" ? "double" : "float";
                const lhs = toNumber(left.value);
                const rhs = toNumber(right.value);
                if (op === "==" || op === "!=" || op === "<" || op === "<=" || op === ">" || op === ">=") {
                    let ok = false;
                    if (Number.isNaN(lhs) || Number.isNaN(rhs)) {
                        ok = op === "!=";
                    }
                    else if (op === "==")
                        ok = lhs === rhs;
                    else if (op === "!=")
                        ok = lhs !== rhs;
                    else if (op === "<")
                        ok = lhs < rhs;
                    else if (op === "<=")
                        ok = lhs <= rhs;
                    else if (op === ">")
                        ok = lhs > rhs;
                    else
                        ok = lhs >= rhs;
                    return makeRvalue(ok ? 1n : 0n, "int");
                }
                let out;
                if (op === "+")
                    out = lhs + rhs;
                else if (op === "-")
                    out = lhs - rhs;
                else if (op === "*")
                    out = lhs * rhs;
                else if (op === "/")
                    out = lhs / rhs;
                else
                    return makeCompileError("That expression is not valid here.");
                const cast = resultBase === "float" ? Math.fround(out) : out;
                const nanSign = Number.isNaN(cast)
                    ? left.nanSign ?? right.nanSign ?? 1
                    : undefined;
                return makeRvalue(cast, resultBase, 0, "", nanSign);
            }
            if (!isIntegerBase(left.base) || !isIntegerBase(right.base)) {
                return makeCompileError("That expression is not valid here.");
            }
            const leftBase = op === "<<" || op === ">>"
                ? integerPromotionBase(left.base)
                : usualIntegerBase(left.base, right.base);
            const rightBase = op === "<<" || op === ">>"
                ? integerPromotionBase(right.base)
                : leftBase;
            const leftNorm = normalizeIntegerForBase(left.value, leftBase);
            const rightNorm = normalizeIntegerForBase(right.value, rightBase);
            if (leftNorm == null || rightNorm == null) {
                return makeCompileError("That expression is not valid here.");
            }
            const leftMeta = integerMetaForBase(leftBase);
            const rightMeta = integerMetaForBase(rightBase);
            if (!leftMeta || !rightMeta) {
                return makeCompileError("That expression is not valid here.");
            }
            if (op === "==" || op === "!=" || op === "<" || op === "<=" || op === ">" || op === ">=") {
                const common = usualIntegerBase(left.base, right.base);
                const lhs = normalizeIntegerForBase(left.value, common);
                const rhs = normalizeIntegerForBase(right.value, common);
                if (lhs == null || rhs == null)
                    return makeCompileError("That expression is not valid here.");
                let ok = false;
                if (op === "==")
                    ok = lhs === rhs;
                else if (op === "!=")
                    ok = lhs !== rhs;
                else if (op === "<")
                    ok = lhs < rhs;
                else if (op === "<=")
                    ok = lhs <= rhs;
                else if (op === ">")
                    ok = lhs > rhs;
                else
                    ok = lhs >= rhs;
                return makeRvalue(ok ? 1n : 0n, "int");
            }
            if (op === "/" || op === "%") {
                if (rightNorm === 0n)
                    return makeUbError("Division by 0 is undefined.");
            }
            if (op === "<<" || op === ">>") {
                const width = bitWidthForBase(leftBase);
                if (!width)
                    return makeCompileError("That expression is not valid here.");
                if (rightNorm < 0n || rightNorm >= BigInt(width)) {
                    return makeUbError("That shift is undefined.");
                }
                if (op === "<<") {
                    if (leftMeta.signed && leftNorm < 0n) {
                        return makeUbError("That shift is undefined.");
                    }
                    if (leftMeta.signed) {
                        const shifted = leftNorm << rightNorm;
                        const overflow = checkIntegerRange(shifted, leftBase);
                        if (overflow)
                            return overflow;
                        return makeRvalue(shifted, leftBase);
                    }
                    const shiftedBits = toUnsignedBits(leftNorm, leftBase) << rightNorm;
                    const wrapped = normalizeIntegerForBase(shiftedBits, leftBase);
                    if (wrapped == null)
                        return makeCompileError("That expression is not valid here.");
                    return makeRvalue(wrapped, leftBase);
                }
                if (!leftMeta.signed) {
                    const shifted = toUnsignedBits(leftNorm, leftBase) >> rightNorm;
                    const wrapped = normalizeIntegerForBase(shifted, leftBase);
                    if (wrapped == null)
                        return makeCompileError("That expression is not valid here.");
                    return makeRvalue(wrapped, leftBase);
                }
                const shifted = leftNorm >> rightNorm;
                const wrapped = normalizeIntegerForBase(shifted, leftBase);
                if (wrapped == null)
                    return makeCompileError("That expression is not valid here.");
                return makeRvalue(wrapped, leftBase);
            }
            if (op === "&" || op === "^" || op === "|") {
                const common = usualIntegerBase(left.base, right.base);
                const lhs = normalizeIntegerForBase(left.value, common);
                const rhs = normalizeIntegerForBase(right.value, common);
                if (lhs == null || rhs == null)
                    return makeCompileError("That expression is not valid here.");
                const lhsBits = toUnsignedBits(lhs, common);
                const rhsBits = toUnsignedBits(rhs, common);
                const outBits = op === "&" ? lhsBits & rhsBits : op === "^" ? lhsBits ^ rhsBits : lhsBits | rhsBits;
                const wrapped = normalizeIntegerForBase(outBits, common);
                if (wrapped == null)
                    return makeCompileError("That expression is not valid here.");
                return makeRvalue(wrapped, common);
            }
            const common = usualIntegerBase(left.base, right.base);
            const lhs = normalizeIntegerForBase(left.value, common);
            const rhs = normalizeIntegerForBase(right.value, common);
            if (lhs == null || rhs == null)
                return makeCompileError("That expression is not valid here.");
            const commonMeta = integerMetaForBase(common);
            if (!commonMeta)
                return makeCompileError("That expression is not valid here.");
            let out;
            if (op === "+")
                out = lhs + rhs;
            else if (op === "-")
                out = lhs - rhs;
            else if (op === "*")
                out = lhs * rhs;
            else if (op === "/") {
                if (commonMeta.signed && rhs === -1n) {
                    const range = integerRangeForBase(common);
                    if (range && lhs === range.min)
                        return integerOverflowError(common);
                }
                if (commonMeta.signed)
                    out = lhs / rhs;
                else
                    out = toUnsignedBits(lhs, common) / toUnsignedBits(rhs, common);
            }
            else if (op === "%") {
                if (commonMeta.signed)
                    out = lhs % rhs;
                else
                    out = toUnsignedBits(lhs, common) % toUnsignedBits(rhs, common);
            }
            else {
                return makeCompileError("That expression is not valid here.");
            }
            if (commonMeta.signed) {
                const overflow = checkIntegerRange(out, common);
                if (overflow)
                    return overflow;
            }
            const normalized = normalizeIntegerForBase(out, common);
            if (normalized == null)
                return makeCompileError("That expression is not valid here.");
            return makeRvalue(normalized, common);
        }
        function incrementLvalue(target, delta, returnUpdated) {
            if (isEvalError(target))
                return target;
            if (target.kind !== "lvalue") {
                return makeCompileError("That expression is not valid here.");
            }
            if (target.isArray) {
                return makeCompileError("That expression is not valid here.");
            }
            const current = runtimeFromEval(target, true);
            if (isEvalError(current))
                return current;
            let updated;
            if (current.depth > 0) {
                const step = pointerStepSize(current.base, current.depth, current.pointeeArrayDims, current.pointeeInnerDepth);
                const next = current.value + BigInt(delta) * step;
                updated = makeRvalue(next, current.base, current.depth, "", undefined, current.pointeeArrayDims, current.pointeeInnerDepth);
            }
            else if (isFloatingBase(current.base)) {
                const nextNum = toNumber(current.value) + delta;
                const cast = current.base === "float" ? Math.fround(nextNum) : nextNum;
                updated = makeRvalue(cast, current.base, 0, "", current.nanSign);
            }
            else if (isIntegerBase(current.base)) {
                const nextRaw = current.value + BigInt(delta);
                const range = integerRangeForBase(current.base);
                if (range?.signed) {
                    const overflow = checkIntegerRange(nextRaw, current.base);
                    if (overflow)
                        return overflow;
                }
                const wrapped = normalizeIntegerForBase(nextRaw, current.base);
                if (wrapped == null)
                    return makeCompileError("That expression is not valid here.");
                updated = makeRvalue(wrapped, current.base);
            }
            else {
                return makeCompileError("That expression is not valid here.");
            }
            if (isEvalError(updated))
                return updated;
            const assigned = assignIntoTarget(target, updated);
            if (isEvalError(assigned))
                return assigned;
            if (returnUpdated)
                return assigned;
            return makeRvalue(current.value, current.base, current.depth, current.label, current.nanSign, current.pointeeArrayDims, current.pointeeInnerDepth);
        }
        function evalNode(node) {
            if (!node)
                return {
                    error: "That expression is not valid here.",
                    kind: "compile",
                };
            if (node.kind === "num") {
                const parsed = parseNumericLiteralValue(node.value);
                if ("error" in parsed)
                    return parsed;
                return makeRvalue(parsed.value, parsed.base, 0, "", parsed.nanSign);
            }
            if (node.kind === "cast") {
                const target = parseType(node.targetType || "int");
                const targetBase = target.base;
                const targetDepth = Number.isFinite(target.depth) ? target.depth : 0;
                const targetPointeeArrayDims = normalizeArrayDims(target.pointeeArrayDims);
                const targetPointeeInnerDepth = normalizePointeeInnerDepth(target.pointeeInnerDepth, targetDepth, targetPointeeArrayDims);
                if (!targetBase || !Number.isFinite(targetDepth)) {
                    return makeCompileError("That expression is not valid here.");
                }
                const rhs = evalNode(node.expr);
                if (isEvalError(rhs))
                    return rhs;
                const source = decayArrayValue(rhs);
                if (isEvalError(source))
                    return source;
                if (targetDepth > 0) {
                    if (source.depth > 0) {
                        const runtime = runtimeFromEval(source, requireValue);
                        if (isEvalError(runtime))
                            return runtime;
                        return makeRvalue(runtime.value, targetBase, targetDepth, "", runtime.nanSign, targetPointeeArrayDims, targetPointeeInnerDepth);
                    }
                    const scalar = coerceScalarResult(source, requireValue);
                    if (isScalarError(scalar))
                        return scalar;
                    if (!isIntegerBase(scalar.base)) {
                        return makeCompileError("That expression is not valid here.");
                    }
                    const asInt = typeof scalar.value === "bigint"
                        ? scalar.value
                        : BigInt(Math.trunc(Number(scalar.value)));
                    return makeRvalue(asInt, targetBase, targetDepth, "", scalar.nanSign, targetPointeeArrayDims, targetPointeeInnerDepth);
                }
                if (source.depth > 0) {
                    const runtime = runtimeFromEval(source, requireValue);
                    if (isEvalError(runtime))
                        return runtime;
                    if (isFloatingBase(targetBase)) {
                        const num = Number(runtime.value);
                        const castNum = targetBase === "float" ? Math.fround(num) : num;
                        return makeRvalue(castNum, targetBase, 0);
                    }
                    if (!isIntegerBase(targetBase)) {
                        return makeCompileError("That expression is not valid here.");
                    }
                    const converted = convertScalarForAssignment(runtime.value, "unsigned long long", node.targetType, runtime.nanSign);
                    if (!converted || "error" in converted) {
                        return makeCompileError("That expression is not valid here.");
                    }
                    return makeRvalue(converted.value, targetBase, 0, "", converted.nanSign);
                }
                const scalar = coerceScalarResult(source, requireValue);
                if (isScalarError(scalar))
                    return scalar;
                const converted = convertScalarForAssignment(scalar.value, scalar.base, node.targetType, scalar.nanSign);
                if (!converted || "error" in converted) {
                    return makeCompileError("That expression is not valid here.");
                }
                return makeRvalue(converted.value, targetBase, 0, "", converted.nanSign);
            }
            if (node.kind === "var") {
                const box = by[node.name];
                if (box)
                    return makeLvalue(box, node.name);
                const arrayInfo = arraysByName.get(node.name);
                if (arrayInfo)
                    return makeArrayLvalue(arrayInfo, node.name);
                return {
                    error: `You can't use ${node.name} before declaring it.`,
                    kind: "compile",
                };
            }
            if (node.kind === "postfix") {
                if (node.op === "++")
                    return incrementLvalue(evalNode(node.expr), 1, false);
                return incrementLvalue(evalNode(node.expr), -1, false);
            }
            if (node.kind === "subscript") {
                const left = evalNode(node.left);
                if (isEvalError(left))
                    return left;
                const index = evalNode(node.index);
                if (isEvalError(index))
                    return index;
                const leftRuntime = runtimeFromEval(left, true);
                if (isEvalError(leftRuntime))
                    return leftRuntime;
                const indexRuntime = runtimeFromEval(index, true);
                if (isEvalError(indexRuntime))
                    return indexRuntime;
                let pointer = leftRuntime;
                let scalar = indexRuntime;
                if (!(pointer.depth > 0 && scalar.depth === 0 && isIntegerBase(scalar.base))) {
                    if (indexRuntime.depth > 0 &&
                        leftRuntime.depth === 0 &&
                        isIntegerBase(leftRuntime.base)) {
                        pointer = indexRuntime;
                        scalar = leftRuntime;
                    }
                    else {
                        return makeCompileError("That expression is not valid here.");
                    }
                }
                const step = pointerStepSize(pointer.base, pointer.depth, pointer.pointeeArrayDims, pointer.pointeeInnerDepth);
                const nextAddress = pointer.value + scalar.value * step;
                const target = state.find((box) => String(box.address ?? "").trim() === String(nextAddress));
                if (!target) {
                    return {
                        error: "That expression is not valid here.",
                        kind: "ub",
                    };
                }
                if (!pointerCanReferenceTarget(pointer, target)) {
                    return {
                        error: "That expression is not valid here.",
                        kind: "ub",
                    };
                }
                if (pointer.pointeeArrayDims.length > 0) {
                    const shape = pointer.pointeeArrayDims.slice();
                    const targetParsedType = parseType(target.type);
                    const targetPointeeInnerDepth = normalizePointeeInnerDepth(target.pointeeInnerDepth ?? targetParsedType.pointeeInnerDepth, Number.isFinite(targetParsedType.depth) ? targetParsedType.depth : 0, normalizeArrayDims(target.pointeeArrayDims));
                    const decayDims = shape.length > 1
                        ? shape.slice(1)
                        : normalizeArrayDims(target.pointeeArrayDims);
                    return {
                        kind: "lvalue",
                        base: pointer.base,
                        depth: pointer.depth,
                        value: nextAddress,
                        address: String(nextAddress),
                        label: `${left.label || ""}[${index.label || ""}]`,
                        isArray: true,
                        arrayShape: shape,
                        pointeeArrayDims: decayDims,
                        pointeeInnerDepth: shape.length > 1
                            ? Math.max(0, pointer.depth - 1)
                            : targetPointeeInnerDepth,
                    };
                }
                return makeLvalue(target, `${left.label || ""}[${index.label || ""}]`);
            }
            if (node.kind === "unary") {
                if (node.op === "++")
                    return incrementLvalue(evalNode(node.expr), 1, true);
                if (node.op === "--")
                    return incrementLvalue(evalNode(node.expr), -1, true);
                const rhs = evalNode(node.expr);
                if (isEvalError(rhs))
                    return rhs;
                const rhsDepth = rhs.depth ?? 0;
                if (node.op === "&") {
                    const label = `&${rhs.label || ""}`;
                    if (rhs.kind !== "lvalue") {
                        return { error: `${label} is not valid here.`, kind: "compile" };
                    }
                    if (rhs.isArray) {
                        const shape = normalizeArrayDims(rhs.arrayShape);
                        if (!shape.length) {
                            return { error: `${label} is not valid here.`, kind: "compile" };
                        }
                        return makeRvalue(rhs.value, rhs.base || "int", rhsDepth, label, rhs.nanSign, shape, Math.max(0, rhsDepth - 1));
                    }
                    if (!rhs.address) {
                        return { error: `${label} is not valid here.`, kind: "compile" };
                    }
                    const nextDepth = Number.isFinite(rhsDepth) ? rhsDepth + 1 : 1;
                    const nextBase = rhs.base || "int";
                    return makeRvalue(String(rhs.address), nextBase, nextDepth, label, rhs.nanSign, normalizeArrayDims(rhs.pointeeArrayDims), rhs.pointeeInnerDepth);
                }
                if (node.op === "*") {
                    const label = `*${rhs.label || ""}`;
                    if (!Number.isFinite(rhsDepth) || rhsDepth < 1) {
                        return {
                            error: `${label} is not a valid dereference.`,
                            kind: "compile",
                        };
                    }
                    const ptrRaw = rhs.value;
                    if (requireValue && String(ptrRaw ?? "") === "") {
                        const sourceLabel = rhs.label || "That pointer";
                        return {
                            error: `${sourceLabel} doesn't have a value yet, so it can't be dereferenced.`,
                            kind: "ub",
                        };
                    }
                    const ptrVal = String(ptrRaw ?? "").trim();
                    if (ptrVal === "") {
                        const sourceLabel = rhs.label || "That pointer";
                        return {
                            error: `${sourceLabel} doesn't have a value yet, so it can't be dereferenced.`,
                            kind: "ub",
                        };
                    }
                    const target = state.find((b) => (b.address ?? "") === ptrVal);
                    if (!target) {
                        return {
                            error: `${label} doesn't point to a known variable.`,
                            kind: "ub",
                        };
                    }
                    if (!pointerCanReferenceTarget({
                        base: rhs.base || "int",
                        depth: rhsDepth,
                        pointeeArrayDims: normalizeArrayDims(rhs.pointeeArrayDims),
                        pointeeInnerDepth: rhs.pointeeInnerDepth,
                    }, target)) {
                        return {
                            error: `${label} has an incompatible pointee type.`,
                            kind: "ub",
                        };
                    }
                    const pointeeArrayDims = normalizeArrayDims(rhs.pointeeArrayDims);
                    if (pointeeArrayDims.length > 0) {
                        const targetParsedType = parseType(target.type);
                        const targetPointeeInnerDepth = normalizePointeeInnerDepth(target.pointeeInnerDepth ?? targetParsedType.pointeeInnerDepth, Number.isFinite(targetParsedType.depth) ? targetParsedType.depth : 0, normalizeArrayDims(target.pointeeArrayDims));
                        return {
                            kind: "lvalue",
                            base: rhs.base || "int",
                            depth: rhsDepth,
                            value: BigInt(ptrVal),
                            address: ptrVal,
                            label,
                            isArray: true,
                            arrayShape: pointeeArrayDims.slice(),
                            pointeeArrayDims: pointeeArrayDims.length > 1
                                ? pointeeArrayDims.slice(1)
                                : normalizeArrayDims(target.pointeeArrayDims),
                            pointeeInnerDepth: pointeeArrayDims.length > 1
                                ? Math.max(0, rhsDepth - 1)
                                : targetPointeeInnerDepth,
                        };
                    }
                    return makeLvalue(target, label);
                }
                if (node.op === "!") {
                    const runtime = runtimeFromEval(rhs, true);
                    if (isEvalError(runtime))
                        return runtime;
                    return makeRvalue(runtimeTruthy(runtime) ? 0n : 1n, "int");
                }
                const scalar = coerceScalarResult(rhs, requireValue);
                if (isScalarError(scalar))
                    return scalar;
                if (node.op === "~") {
                    if (isFloatingBase(scalar.base)) {
                        return {
                            error: "Bitwise operators require integer values.",
                            kind: "compile",
                        };
                    }
                    const promotedBase = integerPromotionBase(scalar.base);
                    const value = normalizeIntegerForBase(scalar.value, promotedBase);
                    if (value == null)
                        return makeCompileError("That expression is not valid here.");
                    const bits = toUnsignedBits(value, promotedBase);
                    const width = bitWidthForBase(promotedBase);
                    if (!width)
                        return makeCompileError("That expression is not valid here.");
                    const mask = (1n << BigInt(width)) - 1n;
                    const out = normalizeIntegerForBase((~bits) & mask, promotedBase);
                    if (out == null)
                        return makeCompileError("That expression is not valid here.");
                    return makeRvalue(out, promotedBase);
                }
                if (node.op === "+")
                    return makeRvalue(scalar.value, isIntegerBase(scalar.base) ? integerPromotionBase(scalar.base) : scalar.base, 0, "", scalar.nanSign);
                if (node.op === "-") {
                    if (isFloatingBase(scalar.base) && Number.isNaN(scalar.value)) {
                        const flipped = scalar.nanSign === -1 ? 1 : -1;
                        return makeRvalue(scalar.value, scalar.base, 0, "", flipped);
                    }
                    if (!isFloatingBase(scalar.base)) {
                        const promotedBase = integerPromotionBase(scalar.base);
                        const value = normalizeIntegerForBase(scalar.value, promotedBase);
                        if (value == null)
                            return makeCompileError("That expression is not valid here.");
                        const range = integerRangeForBase(promotedBase);
                        if (range && value === range.min) {
                            return integerOverflowError(promotedBase);
                        }
                        const neg = normalizeIntegerForBase(-value, promotedBase);
                        if (neg == null)
                            return makeCompileError("That expression is not valid here.");
                        return makeRvalue(neg, promotedBase);
                    }
                    return makeRvalue(-scalar.value, scalar.base);
                }
                return {
                    error: "That expression is not valid here.",
                    kind: "compile",
                };
            }
            if (node.kind === "binary") {
                if (node.op === "&&" || node.op === "||") {
                    const left = evalNode(node.left);
                    if (isEvalError(left))
                        return left;
                    const leftRuntime = runtimeFromEval(left, true);
                    if (isEvalError(leftRuntime))
                        return leftRuntime;
                    const leftTruthy = runtimeTruthy(leftRuntime);
                    if (node.op === "&&" && !leftTruthy)
                        return makeRvalue(0n, "int");
                    if (node.op === "||" && leftTruthy)
                        return makeRvalue(1n, "int");
                    const right = evalNode(node.right);
                    if (isEvalError(right))
                        return right;
                    const rightRuntime = runtimeFromEval(right, true);
                    if (isEvalError(rightRuntime))
                        return rightRuntime;
                    return makeRvalue(runtimeTruthy(rightRuntime) ? 1n : 0n, "int");
                }
                const left = evalNode(node.left);
                if (isEvalError(left))
                    return left;
                const right = evalNode(node.right);
                if (isEvalError(right))
                    return right;
                return evaluateBinaryResolved(node.op, left, right);
            }
            if (node.kind === "assign") {
                const lhs = evalNode(node.left);
                if (isEvalError(lhs))
                    return lhs;
                if (lhs.kind !== "lvalue") {
                    return makeCompileError("That assignment is not valid here.");
                }
                if (node.op === "=") {
                    const rhs = evalNode(node.right);
                    if (isEvalError(rhs))
                        return rhs;
                    return assignIntoTarget(lhs, rhs);
                }
                const rhs = evalNode(node.right);
                if (isEvalError(rhs))
                    return rhs;
                const binaryOp = (() => {
                    if (node.op === "+=")
                        return "+";
                    if (node.op === "-=")
                        return "-";
                    if (node.op === "*=")
                        return "*";
                    if (node.op === "/=")
                        return "/";
                    if (node.op === "%=")
                        return "%";
                    if (node.op === "<<=")
                        return "<<";
                    if (node.op === ">>=")
                        return ">>";
                    if (node.op === "&=")
                        return "&";
                    if (node.op === "^=")
                        return "^";
                    return "|";
                })();
                const lhsValue = runtimeFromEval(lhs, true);
                if (isEvalError(lhsValue))
                    return lhsValue;
                const lhsAsRvalue = makeRvalue(lhsValue.value, lhsValue.base, lhsValue.depth, lhsValue.label, lhsValue.nanSign, lhsValue.pointeeArrayDims);
                const combined = evaluateBinaryResolved(binaryOp, lhsAsRvalue, rhs);
                if (isEvalError(combined))
                    return combined;
                return assignIntoTarget(lhs, combined);
            }
            return { error: "That expression is not valid here.", kind: "compile" };
        }
        return evalNode(expr);
    }
    function evaluateExpression(expr, state, opts = {}) {
        const { requireValue = requireSourceValue } = opts;
        const evaluated = evaluateExpressionRaw(expr, state, opts);
        if (isEvalError(evaluated))
            return evaluated;
        const scalar = coerceScalarResult(evaluated, requireValue);
        if (isScalarError(scalar))
            return scalar;
        return scalar;
    }
    function evaluateCondition(expr, state) {
        const rawEvaluated = evaluateExpressionRaw(expr, state, {
            targetType: "double",
        });
        if (isEvalError(rawEvaluated))
            return rawEvaluated;
        const evaluated = decayArrayValue(rawEvaluated);
        if (isEvalError(evaluated))
            return evaluated;
        if (requireSourceValue && String(evaluated.value ?? "") === "") {
            const label = evaluated.label || "That value";
            return { error: `${label} doesn't have a value yet.`, kind: "ub" };
        }
        if (evaluated.depth > 0) {
            try {
                const addr = BigInt(String(evaluated.value ?? "").trim() || "0");
                return { value: addr !== 0n };
            }
            catch {
                return { error: "That expression is not valid here.", kind: "compile" };
            }
        }
        const base = evaluated.base || "int";
        if (isFloatingBase(base)) {
            const parsed = parseDoubleValueWithSign(evaluated.value);
            if (!parsed)
                return { error: "That expression is not valid here.", kind: "compile" };
            return { value: parsed.value !== 0 };
        }
        try {
            const intVal = typeof evaluated.value === "bigint"
                ? evaluated.value
                : BigInt(String(evaluated.value ?? "").trim() || "0");
            return { value: intVal !== 0n };
        }
        catch {
            return { error: "That expression is not valid here.", kind: "compile" };
        }
    }
    function prependArrayDimension(typeText, length) {
        const dim = Math.max(0, Math.floor(Number(length)));
        const clean = String(typeText || "").trim() || "int";
        const parsed = parseType(clean);
        if (parsed.base &&
            Number.isFinite(parsed.depth) &&
            Array.isArray(parsed.arrayDims) &&
            parsed.arrayDims.length > 0) {
            const dims = [dim, ...parsed.arrayDims]
                .map((value) => `[${value}]`)
                .join("");
            return `${parsed.base}${"*".repeat(Math.max(0, Math.floor(parsed.depth)))}${dims}`;
        }
        const ptrArrayMatch = /^(.+?)\(\s*(\*+)\s*\)\s*((?:\[\s*\d+\s*\]\s*)+)\s*$/.exec(clean);
        if (ptrArrayMatch) {
            const left = String(ptrArrayMatch[1] || "").trimEnd();
            const stars = String(ptrArrayMatch[2] || "");
            const dims = String(ptrArrayMatch[3] || "").replace(/\s+/g, "");
            return `${left} (${stars}[${dim}])${dims}`;
        }
        return `${clean}[${dim}]`;
    }
    function arrayExpressionType(result) {
        if (!result.isArray)
            return null;
        const shape = normalizeArrayDims(result.arrayShape);
        if (!shape.length)
            return null;
        const depth = Number.isFinite(result.depth) && result.depth !== undefined
            ? Math.floor(result.depth)
            : 0;
        const pointeeArrayDims = normalizeArrayDims(result.pointeeArrayDims);
        const pointeeInnerDepth = normalizePointeeInnerDepth(result.pointeeInnerDepth, depth, pointeeArrayDims);
        const elementType = makePointerType(Math.max(0, depth - 1), result.base || "int", pointeeArrayDims, pointeeInnerDepth) ||
            `${result.base || "int"}${"*".repeat(Math.max(0, depth - 1))}`;
        return prependArrayDimension(elementType, shape[0]);
    }
    function evaluateExpressionText(expr, state, opts = {}) {
        const { allowSideEffects = true } = opts;
        const tokens = tokenizeProgram(expr || "");
        if (!tokens.length) {
            return { error: "Enter an expression to evaluate.", kind: "compile" };
        }
        if (tokens.some((t) => t.type === "unknown")) {
            return {
                error: "That line has a character that does not belong in an expression.",
                kind: "compile",
            };
        }
        if (tokens.some((t) => t.type === "sym" && t.value === ";")) {
            return { error: "That expression is not valid here.", kind: "compile" };
        }
        const parsed = parseExpressionTokens(tokens, 0, { allowVars: true });
        if (!parsed || parsed.nextIndex !== tokens.length) {
            return { error: "That expression is not valid here.", kind: "compile" };
        }
        let sawSideEffect = false;
        const evalState = allowSideEffects ? state : cloneBoxes(state);
        const rawEvaluated = evaluateExpressionRaw(parsed.expr, evalState, {
            targetType: "double",
            onSideEffect: () => {
                sawSideEffect = true;
            },
        });
        if (isEvalError(rawEvaluated))
            return rawEvaluated;
        if (!allowSideEffects && sawSideEffect) {
            return {
                error: "Expressions with side effects are not allowed here.",
                kind: "compile",
            };
        }
        const evaluated = rawEvaluated;
        const resultType = evaluated.isArray
            ? arrayExpressionType(evaluated) ||
                makePointerType(Number.isFinite(evaluated.depth) ? evaluated.depth : 0, evaluated.base || "int", normalizeArrayDims(evaluated.pointeeArrayDims), evaluated.pointeeInnerDepth)
            : makePointerType(Number.isFinite(evaluated.depth) ? evaluated.depth : 0, evaluated.base || "int", normalizeArrayDims(evaluated.pointeeArrayDims), evaluated.pointeeInnerDepth);
        const resultKind = evaluated.isArray ? "rvalue" : evaluated.kind || "rvalue";
        const resultAddress = !evaluated.isArray && evaluated.kind === "lvalue" ? evaluated.address : "";
        return {
            result: {
                kind: resultKind,
                type: resultType || "int",
                value: evaluated.value,
                address: resultAddress,
                nanSign: evaluated.nanSign,
            },
        };
    }
    function convertScalarForAssignment(value, base, targetType, nanSign) {
        const { base: targetBase, depth } = parseType(targetType);
        if (!targetBase || depth !== 0)
            return null;
        if (isFloatingBase(targetBase)) {
            const num = isFloatingBase(base)
                ? typeof value === "bigint"
                    ? Number(value)
                    : value
                : Number(value);
            const cast = targetBase === "float" ? Math.fround(num) : num;
            const nextNanSign = Number.isNaN(cast) ? (nanSign ?? 1) : undefined;
            return { value: cast, base: targetBase, nanSign: nextNanSign };
        }
        if (!isIntegerBase(targetBase))
            return null;
        if (targetBase === "bool") {
            if (isFloatingBase(base)) {
                const num = typeof value === "bigint" ? Number(value) : value;
                return { value: num === 0 ? 0n : 1n, base: targetBase };
            }
            return { value: value === 0n ? 0n : 1n, base: targetBase };
        }
        let intValue;
        if (isFloatingBase(base)) {
            const num = typeof value === "bigint" ? Number(value) : value;
            if (!Number.isFinite(num))
                return integerOverflowError(targetBase);
            try {
                intValue = BigInt(Math.trunc(num));
            }
            catch {
                return integerOverflowError(targetBase);
            }
            const range = integerRangeForBase(targetBase);
            if (range && (intValue < range.min || intValue > range.max)) {
                return integerOverflowError(targetBase);
            }
        }
        else {
            intValue = typeof value === "bigint" ? value : BigInt(Math.trunc(value));
        }
        const wrapped = wrapIntegerToBase(intValue, targetBase);
        if (wrapped == null)
            return null;
        return { value: wrapped, base: targetBase };
    }
    function convertAssignmentValue(evaluated, targetType, requireValue) {
        const source = decayArrayValue(evaluated);
        if (isEvalError(source))
            return source;
        const { base: targetBase, depth: targetDepth, pointeeArrayDims: targetPointeeArrayDimsRaw, pointeeInnerDepth: targetPointeeInnerDepthRaw, } = parseType(targetType);
        if (!targetBase || !Number.isFinite(targetDepth)) {
            return { error: "That assignment is not valid here.", kind: "compile" };
        }
        const targetPointeeArrayDims = normalizeArrayDims(targetPointeeArrayDimsRaw);
        const targetPointeeInnerDepth = normalizePointeeInnerDepth(targetPointeeInnerDepthRaw, targetDepth, targetPointeeArrayDims);
        if (targetDepth === 0) {
            const scalar = coerceScalarResult(source, requireValue);
            if (isScalarError(scalar))
                return scalar;
            const converted = convertScalarForAssignment(scalar.value, scalar.base, targetType, scalar.nanSign);
            if (!converted) {
                return { error: "That assignment is not valid here.", kind: "compile" };
            }
            if ("error" in converted)
                return converted;
            return {
                value: formatValueForType(converted.value, targetType, {
                    nanSign: converted.nanSign,
                }),
                nanSign: converted.nanSign,
            };
        }
        const evalDepth = Number.isFinite(source.depth) && source.depth !== undefined
            ? source.depth
            : 0;
        const evalBase = source.base || "int";
        const evalPointeeArrayDims = normalizeArrayDims(source.pointeeArrayDims);
        const evalPointeeInnerDepth = normalizePointeeInnerDepth(source.pointeeInnerDepth, evalDepth, evalPointeeArrayDims);
        if (evalDepth === 0 && isIntegerBase(evalBase)) {
            try {
                const rawInt = typeof source.value === "bigint"
                    ? source.value
                    : BigInt(String(source.value ?? "").trim() || "0");
                if (rawInt === 0n) {
                    return { value: "0", nanSign: source.nanSign };
                }
            }
            catch {
                return { error: "That assignment is not valid here.", kind: "compile" };
            }
        }
        if (evalDepth !== targetDepth ||
            evalBase !== targetBase ||
            !samePointerPointeeType(evalDepth, evalPointeeArrayDims, evalPointeeInnerDepth, targetDepth, targetPointeeArrayDims, targetPointeeInnerDepth)) {
            const expectedType = makePointerType(evalDepth, evalBase, evalPointeeArrayDims, evalPointeeInnerDepth) ||
                `int${"*".repeat(evalDepth)}`;
            return { kind: "type-mismatch", expectedType };
        }
        if (requireValue && String(source.value ?? "") === "") {
            const label = source.label || "That value";
            return { error: `${label} doesn't have a value yet.`, kind: "ub" };
        }
        return {
            value: formatValueForType(source.value, targetType, {
                nanSign: source.nanSign,
            }),
            nanSign: source.nanSign,
        };
    }
    function validateAssignmentExpr(state, targetType, targetName, expr) {
        const evaluated = evaluateExpressionRaw(expr, cloneBoxes(state), {
            targetType,
        });
        const converted = convertAssignmentValue(evaluated, targetType, requireSourceValue);
        if ("kind" in converted && converted.kind === "type-mismatch") {
            return typeMismatchError(targetName, converted.expectedType);
        }
        if ("error" in converted)
            return converted;
        return null;
    }
    function applyAssignmentToTarget(boxes, target, targetType, expr) {
        const evaluated = evaluateExpressionRaw(expr, boxes, {
            targetType,
        });
        const converted = convertAssignmentValue(evaluated, targetType, requireSourceValue);
        if (!converted ||
            "error" in converted ||
            ("kind" in converted && converted.kind === "type-mismatch"))
            return null;
        target.value = converted.value;
        return boxes;
    }
    function resolveAssignmentTarget(state, lhs) {
        const evaluated = evaluateExpressionRaw(lhs, state, {
            targetType: "int",
        });
        if (isEvalError(evaluated))
            return evaluated;
        if (evaluated.kind !== "lvalue") {
            return { error: "That assignment is not valid here.", kind: "compile" };
        }
        const { base, depth } = evaluated;
        const targetType = makePointerType(Number.isFinite(depth) ? depth : 0, base || "int", normalizeArrayDims(evaluated.pointeeArrayDims), evaluated.pointeeInnerDepth) || "int";
        const target = state.find((b) => (b.address ?? "") === (evaluated.address ?? ""));
        if (!target) {
            return { error: "That assignment is not valid here.", kind: "compile" };
        }
        return { target, targetType };
    }
    function applyStatement(state, stmt, opts) {
        if (!stmt)
            return state;
        const { alloc = (type) => String(randAddr(type || "int")), allowRedeclare = true, } = opts;
        const boxes = cloneBoxes(state);
        const by = Object.fromEntries(boxes.map((b) => [b.name, b]));
        if (stmt.kind === "decl") {
            const type = stmt.type || "int";
            if (Array.isArray(stmt.arrayShape) && stmt.arrayShape.length > 0) {
                const shape = stmt.arrayShape.map((d) => Math.max(0, Math.floor(Number(d))));
                if (!shape.length || shape.some((d) => d <= 0))
                    return null;
                let redeclare = false;
                forEachArrayIndex(shape, (indices) => {
                    const elementName = arrayElementName(stmt.name, indices);
                    if (by[elementName] && !allowRedeclare)
                        redeclare = true;
                    if (!by[elementName]) {
                        boxes.push({
                            name: elementName,
                            type: stmt.elementType || type,
                            value: "",
                            address: alloc(stmt.elementType || type),
                            arrayRoot: stmt.name,
                            arrayShape: shape.slice(),
                            arrayIndices: indices.slice(),
                            pointeeArrayDims: stmt.elementPointeeArrayDims
                                ? [...stmt.elementPointeeArrayDims]
                                : [],
                            pointeeInnerDepth: stmt.elementPointeeInnerDepth,
                        });
                    }
                });
                if (redeclare)
                    return null;
            }
            else {
                if (by[stmt.name] && !allowRedeclare)
                    return null;
                if (!by[stmt.name]) {
                    boxes.push({
                        name: stmt.name,
                        type,
                        value: "",
                        address: alloc(type),
                        pointeeArrayDims: stmt.pointeeArrayDims
                            ? [...stmt.pointeeArrayDims]
                            : [],
                        pointeeInnerDepth: stmt.pointeeInnerDepth,
                    });
                }
            }
            return boxes;
        }
        if (stmt.kind === "empty") {
            return boxes;
        }
        if (stmt.kind === "assign") {
            const evaluated = evaluateExpressionRaw({
                kind: "assign",
                op: stmt.op,
                left: stmt.lhs,
                right: stmt.rhs,
            }, boxes, {
                targetType: "double",
            });
            if (isEvalError(evaluated))
                return null;
            return boxes;
        }
        if (stmt.kind === "declAssign") {
            const declType = stmt.declType || "int";
            if (by[stmt.name] && !allowRedeclare)
                return null;
            if (!by[stmt.name]) {
                boxes.push({
                    name: stmt.name,
                    type: declType,
                    value: "",
                    address: alloc(declType),
                    pointeeArrayDims: stmt.pointeeArrayDims
                        ? [...stmt.pointeeArrayDims]
                        : [],
                    pointeeInnerDepth: stmt.pointeeInnerDepth,
                });
            }
            const target = by[stmt.name] || boxes.find((b) => b.name === stmt.name);
            if (!target)
                return null;
            return applyAssignmentToTarget(boxes, target, declType, stmt.expr);
        }
        if (stmt.kind === "expr") {
            const evaluated = evaluateExpressionRaw(stmt.expr, boxes, {
                targetType: "double",
            });
            if (isEvalError(evaluated))
                return null;
            return boxes;
        }
        return null;
    }
    function typeMismatchError(name, expectedType) {
        const text = `${name}'s type would need to be ${expectedType} for this line to work.`;
        const html = `<code class="tok-name">${name}</code>'s type would need to be <code class="tok-type">${expectedType}</code> for this line to work.`;
        return { error: { text, html }, kind: "compile" };
    }
    function describeTokensError(tokens, seenDecl) {
        if (!tokens.length)
            return "Line has an error.";
        if (tokens.some((t) => t.type === "unknown" && t.value === "/*"))
            return "Block comment is not closed.";
        if (tokens.some((t) => t.type === "unknown"))
            return "That line has a character that does not belong in a declaration or assignment.";
        if (tokens[0].type === "kw" && tokens[0].value === "if") {
            return 'If statements should look like "if (condition) statement;" or "if (condition) { ... }".';
        }
        if (tokens[0].type === "kw" && tokens[0].value === "while") {
            return 'While statements should look like "while (condition) statement;" or "while (condition) { ... }".';
        }
        if (tokens[0].type === "kw" && tokens[0].value === "else") {
            return 'Else statements should look like "else statement;" or "else { ... }".';
        }
        const decl = parseDeclHead(tokens);
        if (decl.kind === "partial") {
            return "A declaration needs a variable name.";
        }
        if (decl.kind === "full" &&
            Array.isArray(decl.arrayShape) &&
            decl.arrayShape.length > 0 &&
            decl.hasInitializer) {
            return "Array declarations can't have initializers yet.";
        }
        if (decl.kind === "full" && decl.hasInitializer && !Number.isFinite(decl.rhsStart)) {
            return "Declaration needs a value on the right.";
        }
        const parsedExpr = parseExpressionTokens(tokens, 0, { allowVars: true });
        if (parsedExpr && parsedExpr.nextIndex === tokens.length) {
            if (tokens[0]?.type === "ident" && !hasDeclaredPrefix(tokens[0].value, seenDecl)) {
                return `You can't use ${tokens[0].value} before declaring it.`;
            }
            return "Expression statements need a semicolon.";
        }
        return "Line should be a declaration, control statement, or expression statement.";
    }
    function validateStatement(tokens, state, seenDecl, alloc) {
        if (tokens.some((t) => t.type === "unknown")) {
            return {
                error: "That line has a character that does not belong in this statement.",
                kind: "compile",
            };
        }
        const parsed = parseStatementTokens(tokens);
        if (!parsed) {
            return {
                error: describeTokensError(tokens, seenDecl),
                kind: "compile",
            };
        }
        if (parsed.kind === "blockStart" || parsed.kind === "blockEnd") {
            return { parsed, next: state };
        }
        if (parsed.kind === "else") {
            return { parsed, next: state };
        }
        if (parsed.kind === "if") {
            const result = evaluateCondition(parsed.expr, cloneBoxes(state));
            if ("error" in result) {
                return { error: result.error, kind: result.kind };
            }
            return { parsed, next: state };
        }
        if (parsed.kind === "while") {
            const result = evaluateCondition(parsed.expr, cloneBoxes(state));
            if ("error" in result) {
                return { error: result.error, kind: result.kind };
            }
            return { parsed, next: state };
        }
        if (parsed.kind === "decl" || parsed.kind === "declAssign") {
            if (seenDecl.has(parsed.name))
                return {
                    error: `You already declared ${parsed.name}.`,
                    kind: "compile",
                };
            if (parsed.kind === "declAssign") {
                const targetType = parsed.declType || "int";
                const err = validateAssignmentExpr(state, targetType, parsed.name, parsed.expr);
                if (err)
                    return err;
            }
        }
        else if (parsed.kind === "assign") {
            if (parsed.op === "=") {
                const resolved = resolveAssignmentTarget(state, parsed.lhs);
                if ("error" in resolved)
                    return resolved;
                const err = validateAssignmentExpr(state, resolved.target.type, resolved.target.name, parsed.rhs);
                if (err)
                    return err;
            }
            const checked = evaluateExpressionRaw({
                kind: "assign",
                op: parsed.op,
                left: parsed.lhs,
                right: parsed.rhs,
            }, cloneBoxes(state), { targetType: "double" });
            if (isEvalError(checked))
                return checked;
        }
        else if (parsed.kind === "expr") {
            const checked = evaluateExpressionRaw(parsed.expr, cloneBoxes(state), {
                targetType: "double",
            });
            if (isEvalError(checked))
                return checked;
        }
        const next = applyStatement(state, parsed, {
            alloc,
            allowRedeclare: false,
        });
        if (!next)
            return { error: "That statement is not valid here.", kind: "compile" };
        return { next, parsed };
    }
    function isBracePart(part, brace) {
        if (!part?.tokens?.length || part.tokens.length !== 1)
            return false;
        const tok = part.tokens[0];
        if (tok.type !== "sym")
            return false;
        if (brace)
            return tok.value === brace;
        return tok.value === "{" || tok.value === "}";
    }
    function isElsePart(part) {
        if (!part?.tokens?.length || part.tokens.length !== 1)
            return false;
        const tok = part.tokens[0];
        return tok.type === "kw" && tok.value === "else";
    }
    function isDeclarationPart(part) {
        if (!part?.tokens?.length)
            return false;
        const parsed = parseStatementTokens(part.tokens);
        return parsed?.kind === "decl" || parsed?.kind === "declAssign";
    }
    function parseControlStatementMaps(parts, opts = {}) {
        const ifMap = new Map();
        const whileMap = new Map();
        const errors = new Map();
        const incomplete = new Set();
        const usedElse = new Set();
        const fallbackLastLine = parts.length > 0
            ? Number.isFinite(parts[parts.length - 1]?.endLine)
                ? parts[parts.length - 1].endLine
                : 0
            : 0;
        const lastLine = Number.isFinite(opts.lastLine)
            ? Math.max(0, Number(opts.lastLine))
            : Math.max(0, fallbackLastLine);
        const ifDeclarationNeedsBraces = "Variable declarations in if/else statements require braces.";
        const whileDeclarationNeedsBraces = "Variable declarations in while statements require braces.";
        const extentMemo = new Map();
        const ifMemo = new Map();
        const whileMemo = new Map();
        const lineForPart = (part) => Number.isFinite(part?.endLine) ? part.endLine : lastLine;
        function parseStatementExtent(startIndex) {
            if (startIndex >= parts.length) {
                return { kind: "incomplete", line: lastLine };
            }
            const memoized = extentMemo.get(startIndex);
            if (memoized)
                return memoized;
            const part = parts[startIndex];
            if (!part?.tokens?.length) {
                const result = { kind: "ok", endIndex: startIndex };
                extentMemo.set(startIndex, result);
                return result;
            }
            if (isElsePart(part)) {
                const result = {
                    kind: "error",
                    line: lineForPart(part),
                    message: "Else statements must follow an if statement.",
                };
                extentMemo.set(startIndex, result);
                return result;
            }
            const ifParsed = parseIfHeaderTokens(part.tokens);
            if (ifParsed) {
                const result = parseIfAt(startIndex, ifParsed);
                extentMemo.set(startIndex, result);
                return result;
            }
            const whileParsed = parseWhileHeaderTokens(part.tokens);
            if (whileParsed) {
                const result = parseWhileAt(startIndex, whileParsed);
                extentMemo.set(startIndex, result);
                return result;
            }
            if (isBracePart(part, "{")) {
                let depth = 0;
                for (let i = startIndex; i < parts.length; i++) {
                    const probe = parts[i];
                    if (isBracePart(probe, "{")) {
                        depth++;
                        continue;
                    }
                    if (isBracePart(probe, "}")) {
                        depth--;
                        if (depth === 0) {
                            const result = { kind: "ok", endIndex: i };
                            extentMemo.set(startIndex, result);
                            return result;
                        }
                    }
                }
                const result = { kind: "incomplete", line: lastLine };
                extentMemo.set(startIndex, result);
                return result;
            }
            const result = { kind: "ok", endIndex: startIndex };
            extentMemo.set(startIndex, result);
            return result;
        }
        function parseIfAt(headerIndex, ifParsed) {
            const memoized = ifMemo.get(headerIndex);
            if (memoized)
                return memoized;
            const header = parts[headerIndex];
            const headerStartLine = header?.startLine ?? lineForPart(header);
            const headerEndLine = header?.endLine ?? lineForPart(header);
            const openIndex = headerIndex + 1;
            const openPart = parts[openIndex];
            if (!openPart) {
                const result = { kind: "incomplete", line: lastLine };
                ifMemo.set(headerIndex, result);
                return result;
            }
            const thenUsesBraces = isBracePart(openPart, "{");
            if (!thenUsesBraces && isDeclarationPart(openPart)) {
                const immediateElseIndex = openIndex + 1;
                if (isElsePart(parts[immediateElseIndex])) {
                    usedElse.add(immediateElseIndex);
                }
                const result = {
                    kind: "error",
                    line: lineForPart(openPart),
                    message: ifDeclarationNeedsBraces,
                };
                ifMemo.set(headerIndex, result);
                return result;
            }
            const thenExtent = parseStatementExtent(openIndex);
            if (thenExtent.kind !== "ok") {
                ifMemo.set(headerIndex, thenExtent);
                return thenExtent;
            }
            const closeIndex = thenExtent.endIndex;
            let trueTarget = openIndex;
            if (thenUsesBraces && closeIndex > openIndex + 1) {
                trueTarget = openIndex + 1;
            }
            let elseIndex = null;
            let elseOpenIndex = null;
            let elseCloseIndex = null;
            let elseTarget = null;
            let afterIndex = closeIndex + 1;
            const possibleElseIndex = closeIndex + 1;
            const possibleElse = parts[possibleElseIndex];
            if (isElsePart(possibleElse)) {
                usedElse.add(possibleElseIndex);
                elseIndex = possibleElseIndex;
                elseOpenIndex = possibleElseIndex + 1;
                const elseOpenPart = parts[elseOpenIndex];
                if (!elseOpenPart) {
                    const result = {
                        kind: "incomplete",
                        line: lastLine,
                    };
                    ifMemo.set(headerIndex, result);
                    return result;
                }
                const elseUsesBraces = isBracePart(elseOpenPart, "{");
                if (!elseUsesBraces && isDeclarationPart(elseOpenPart)) {
                    const result = {
                        kind: "error",
                        line: lineForPart(elseOpenPart),
                        message: ifDeclarationNeedsBraces,
                    };
                    ifMemo.set(headerIndex, result);
                    return result;
                }
                const elseExtent = parseStatementExtent(elseOpenIndex);
                if (elseExtent.kind !== "ok") {
                    ifMemo.set(headerIndex, elseExtent);
                    return elseExtent;
                }
                elseCloseIndex = elseExtent.endIndex;
                afterIndex = elseCloseIndex + 1;
                elseTarget = elseOpenIndex;
                if (elseUsesBraces && elseCloseIndex >= elseOpenIndex + 1) {
                    elseTarget = elseOpenIndex + 1;
                }
            }
            const falseTarget = elseTarget ??
                (afterIndex < parts.length ? afterIndex : parts.length);
            ifMap.set(headerIndex, {
                headerIndex,
                headerStartLine,
                headerEndLine,
                openIndex,
                closeIndex,
                trueTarget,
                falseTarget,
                elseIndex,
                elseOpenIndex,
                elseCloseIndex,
                elseTarget,
                afterIndex,
                expr: ifParsed.expr,
                hasVar: ifParsed.hasVar,
            });
            const result = {
                kind: "ok",
                endIndex: elseCloseIndex ?? closeIndex,
            };
            ifMemo.set(headerIndex, result);
            return result;
        }
        function parseWhileAt(headerIndex, whileParsed) {
            const memoized = whileMemo.get(headerIndex);
            if (memoized)
                return memoized;
            const header = parts[headerIndex];
            const headerStartLine = header?.startLine ?? lineForPart(header);
            const headerEndLine = header?.endLine ?? lineForPart(header);
            const openIndex = headerIndex + 1;
            const openPart = parts[openIndex];
            if (!openPart) {
                const result = { kind: "incomplete", line: lastLine };
                whileMemo.set(headerIndex, result);
                return result;
            }
            const bodyUsesBraces = isBracePart(openPart, "{");
            if (!bodyUsesBraces && isDeclarationPart(openPart)) {
                const result = {
                    kind: "error",
                    line: lineForPart(openPart),
                    message: whileDeclarationNeedsBraces,
                };
                whileMemo.set(headerIndex, result);
                return result;
            }
            const bodyExtent = parseStatementExtent(openIndex);
            if (bodyExtent.kind !== "ok") {
                whileMemo.set(headerIndex, bodyExtent);
                return bodyExtent;
            }
            const closeIndex = bodyExtent.endIndex;
            let trueTarget = openIndex;
            if (bodyUsesBraces && closeIndex > openIndex + 1) {
                trueTarget = openIndex + 1;
            }
            const afterIndex = closeIndex + 1;
            whileMap.set(headerIndex, {
                headerIndex,
                headerStartLine,
                headerEndLine,
                openIndex,
                closeIndex,
                trueTarget,
                afterIndex,
                expr: whileParsed.expr,
                hasVar: whileParsed.hasVar,
            });
            const result = { kind: "ok", endIndex: closeIndex };
            whileMemo.set(headerIndex, result);
            return result;
        }
        for (let i = 0; i < parts.length; i++) {
            const part = parts[i];
            if (!part?.tokens?.length)
                continue;
            const ifParsed = parseIfHeaderTokens(part.tokens);
            if (ifParsed) {
                const result = parseIfAt(i, ifParsed);
                if (result.kind === "error") {
                    errors.set(result.line, result.message);
                }
                else if (result.kind === "incomplete") {
                    incomplete.add(result.line);
                }
                continue;
            }
            const whileParsed = parseWhileHeaderTokens(part.tokens);
            if (!whileParsed)
                continue;
            const result = parseWhileAt(i, whileParsed);
            if (result.kind === "error") {
                errors.set(result.line, result.message);
            }
            else if (result.kind === "incomplete") {
                incomplete.add(result.line);
            }
        }
        parts.forEach((part, idx) => {
            if (!isElsePart(part))
                return;
            if (usedElse.has(idx))
                return;
            const line = Number.isFinite(part.endLine) ? part.endLine : lastLine;
            errors.set(line, "Else statements must follow an if statement.");
        });
        return { ifMap, whileMap, errors, incomplete };
    }
    function buildIfStatementMap(parts, opts = {}) {
        const parsed = parseControlStatementMaps(parts, opts);
        return {
            map: parsed.ifMap,
            errors: parsed.errors,
            incomplete: parsed.incomplete,
        };
    }
    function buildWhileStatementMap(parts, opts = {}) {
        const parsed = parseControlStatementMaps(parts, opts);
        return {
            map: parsed.whileMap,
            errors: parsed.errors,
            incomplete: parsed.incomplete,
        };
    }
    function buildStatementMap(lines) {
        const text = lines.join("\n");
        const tokens = tokenizeProgram(text);
        const parts = splitStatements(tokens);
        const byLine = new Array(lines.length).fill(null);
        parts.forEach((part) => {
            if (!Number.isFinite(part.startLine) || !Number.isFinite(part.endLine))
                return;
            const range = {
                startLine: part.startLine,
                endLine: part.endLine,
                hasSemicolon: !!part.hasSemicolon,
            };
            for (let i = range.startLine; i <= range.endLine; i++) {
                if (i >= 0 && i < byLine.length && !byLine[i])
                    byLine[i] = range;
            }
        });
        return { parts, byLine };
    }
    function statementRangeForLine(statementMap, lineIndex) {
        if (!statementMap || !Array.isArray(statementMap.byLine))
            return null;
        if (!Number.isFinite(lineIndex))
            return null;
        if (lineIndex < 0 || lineIndex >= statementMap.byLine.length)
            return null;
        return statementMap.byLine[lineIndex];
    }
    function getStatementContext(lines, boundary) {
        const statementMap = buildStatementMap(lines);
        const currentRange = statementRangeForLine(statementMap, boundary);
        const prevRange = statementRangeForLine(statementMap, boundary - 1);
        const currentStart = currentRange?.startLine;
        const currentEnd = currentRange?.endLine;
        const isMultiLine = typeof currentStart === "number" &&
            typeof currentEnd === "number" &&
            Number.isFinite(currentStart) &&
            Number.isFinite(currentEnd) &&
            currentEnd > currentStart;
        const midStatement = isMultiLine && boundary > currentStart && boundary <= currentEnd;
        const atStatementStart = isMultiLine && boundary === currentStart;
        return {
            statementMap,
            currentRange,
            prevRange,
            midStatement,
            atStatementStart,
        };
    }
    function findMissingSemicolonLines(text) {
        const lines = text.split(/\r?\n/);
        const missing = [];
        const patched = [];
        let inBlock = false;
        lines.forEach((line, idx) => {
            const raw = line;
            let i = 0;
            let lastCodeIndex = -1;
            let sawCode = false;
            while (i < raw.length) {
                const ch = raw[i];
                const next = raw[i + 1];
                if (inBlock) {
                    if (ch === "*" && next === "/") {
                        inBlock = false;
                        i += 2;
                        continue;
                    }
                    i += 1;
                    continue;
                }
                if (ch === "/" && next === "/")
                    break;
                if (ch === "/" && next === "*") {
                    inBlock = true;
                    i += 2;
                    continue;
                }
                if (ch === "{" || ch === "}") {
                    i += 1;
                    continue;
                }
                if (!/\s/.test(ch)) {
                    sawCode = true;
                    lastCodeIndex = i;
                }
                i += 1;
            }
            if (!sawCode) {
                patched.push(raw);
                return;
            }
            const clean = stripAllComments(raw).trim();
            if (!clean || clean === "{" || clean === "}") {
                patched.push(raw);
                return;
            }
            if (/^if\b/.test(clean)) {
                patched.push(raw);
                return;
            }
            if (/^while\b/.test(clean)) {
                patched.push(raw);
                return;
            }
            if (/^}\s*else\b/.test(clean)) {
                patched.push(raw);
                return;
            }
            if (/^else\b/.test(clean)) {
                patched.push(raw);
                return;
            }
            if (raw[lastCodeIndex] === ";") {
                patched.push(raw);
                return;
            }
            missing.push(idx + 1);
            patched.push(`${raw.slice(0, lastCodeIndex + 1)};${raw.slice(lastCodeIndex + 1)}`);
        });
        if (!missing.length)
            return [];
        const patchedText = patched.join("\n");
        const state = applyProgram(patchedText);
        if (!state)
            return [];
        return missing;
    }
    function parseStatements(text) {
        const tokens = tokenizeProgram(text);
        const parts = splitStatements(tokens);
        const statements = [];
        for (const part of parts) {
            if (!part.tokens.length)
                continue;
            const parsed = parseStatementTokens(part.tokens);
            if (!parsed)
                continue;
            if (part.hasSemicolon)
                statements.push(parsed);
        }
        return statements;
    }
    function classifyLineStatuses(lines, opts = {}) {
        const invalid = new Set();
        const incomplete = new Set();
        const errors = new Map();
        const errorKinds = new Map();
        const info = new Map();
        const text = lines.join("\n");
        const tokens = tokenizeProgram(text);
        let tokenIndex = 0;
        let currentTokens = [];
        let state = [];
        const alloc = opts.alloc || ((type) => String(randAddr(type || "int")));
        const declared = new Set();
        const scopes = [new Set()];
        const escapeHtml = (value) => String(value)
            .replace(/&/g, "&amp;")
            .replace(/</g, "&lt;")
            .replace(/>/g, "&gt;")
            .replace(/"/g, "&quot;")
            .replace(/'/g, "&#39;");
        const toStatementSnippet = (startLine, startCol, endLine) => {
            const safeStart = Math.max(0, startLine || 0);
            const safeEnd = Math.max(safeStart, endLine || 0);
            const parts = [];
            if (safeStart === safeEnd) {
                parts.push((lines[safeStart] || "").slice(startCol || 0));
            }
            else {
                parts.push((lines[safeStart] || "").slice(startCol || 0));
                for (let i = safeStart + 1; i <= safeEnd; i++) {
                    parts.push(lines[i] || "");
                }
            }
            const joined = parts.join("\n");
            const stripped = stripAllComments(joined);
            return stripped.replace(/\n/g, " ").trim();
        };
        for (let lineIndex = 0; lineIndex < lines.length; lineIndex++) {
            let status = "";
            const applyStatementTokens = (tokensToValidate, endTok, commit = true) => {
                if (!tokensToValidate.length)
                    return;
                const result = validateStatement(tokensToValidate, state, declared, alloc);
                if ("error" in result) {
                    status = "invalid";
                    errors.set(endTok.line, result.error);
                    errorKinds.set(endTok.line, result.kind || "compile");
                    const startLine = tokensToValidate[0]?.line;
                    const startCol = tokensToValidate[0]?.col;
                    const errKind = result.kind || "compile";
                    if (errKind === "compile" &&
                        Number.isFinite(startLine) &&
                        endTok.line > startLine) {
                        const snippet = toStatementSnippet(startLine, startCol, endTok.line);
                        const text = `This statement spans multiple lines and has a compilation error. In C, a line break acts like a space, so your statement is ${snippet}.`;
                        const html = `This statement spans multiple lines and has a compilation error. In C, a line break acts like a space, so your statement is <code class="tok-code">${escapeHtml(snippet)}</code>.`;
                        info.set(endTok.line, { text, html });
                    }
                    return;
                }
                if (commit) {
                    const startLine = tokensToValidate[0]?.line;
                    const startCol = tokensToValidate[0]?.col;
                    if (Number.isFinite(startLine) && endTok.line > startLine) {
                        const snippet = toStatementSnippet(startLine, startCol, endTok.line);
                        const text = `This statement spans multiple lines. In C, a line break acts like a space, so this statement is ${snippet}.`;
                        const html = `This statement spans multiple lines. In C, a line break acts like a space, so this statement is <code class="tok-code">${escapeHtml(snippet)}</code>.`;
                        info.set(endTok.line, { text, html });
                    }
                }
                if (result.parsed.kind === "blockStart") {
                    scopes.push(new Set());
                    return;
                }
                if (result.parsed.kind === "blockEnd") {
                    const popped = popScope(scopes, declared, result.next);
                    if (popped.error) {
                        status = "invalid";
                        errors.set(endTok.line, popped.error);
                        errorKinds.set(endTok.line, "compile");
                        return;
                    }
                    state = popped.state;
                    return;
                }
                if (!commit)
                    return;
                if (result.parsed.kind === "decl" ||
                    result.parsed.kind === "declAssign") {
                    addDeclaredNames(scopes, declared, result.parsed.declaredNames);
                }
                state = result.next;
            };
            const flushControlPrefix = (nextTok) => {
                if (!currentTokens.length)
                    return;
                const splitAfterElse = currentTokens.length === 1 &&
                    currentTokens[0].type === "kw" &&
                    currentTokens[0].value === "else" &&
                    !(nextTok.type === "sym" && nextTok.value === "{");
                const headerEnd = controlHeaderEndIndex(currentTokens);
                const splitAfterControlHeader = headerEnd >= 0 && headerEnd === currentTokens.length - 1;
                if (!splitAfterElse && !splitAfterControlHeader)
                    return;
                const endTok = currentTokens[currentTokens.length - 1];
                applyStatementTokens(currentTokens, endTok, false);
                currentTokens = [];
            };
            while (tokenIndex < tokens.length &&
                tokens[tokenIndex].line === lineIndex) {
                const tok = tokens[tokenIndex];
                flushControlPrefix(tok);
                if (tok.type === "sym" && tok.value === ";") {
                    if (currentTokens.length) {
                        applyStatementTokens(currentTokens, tok);
                        currentTokens = [];
                    }
                    tokenIndex++;
                    continue;
                }
                if (isBraceToken(tok)) {
                    if (currentTokens.length) {
                        const endTok = currentTokens[currentTokens.length - 1];
                        applyStatementTokens(currentTokens, endTok, false);
                        currentTokens = [];
                    }
                    applyStatementTokens([tok], tok);
                    tokenIndex++;
                    continue;
                }
                currentTokens.push(tok);
                tokenIndex++;
            }
            if (status !== "invalid" && currentTokens.length) {
                const lastLine = currentTokens[currentTokens.length - 1]?.line;
                if (lastLine === lineIndex) {
                    const lineText = lines[lineIndex] || "";
                    const allowIntPrefix = !/\s$/.test(lineText);
                    const isPrefix = isStatementPrefix(currentTokens, declared, allowIntPrefix);
                    if (!isPrefix) {
                        status = "invalid";
                        errors.set(lineIndex, describeTokensError(currentTokens, declared));
                        errorKinds.set(lineIndex, "compile");
                    }
                }
            }
            if (status === "invalid")
                invalid.add(lineIndex);
        }
        incomplete.clear();
        const parts = splitStatements(tokens);
        const ifBlocks = buildIfStatementMap(parts, {
            lastLine: Math.max(0, lines.length - 1),
        });
        const whileBlocks = buildWhileStatementMap(parts, {
            lastLine: Math.max(0, lines.length - 1),
        });
        parts.forEach((part, idx) => {
            if (!part?.tokens?.length)
                return;
            if (part.hasSemicolon)
                return;
            if (ifBlocks.map.has(idx))
                return;
            if (whileBlocks.map.has(idx))
                return;
            if (part.tokens[0]?.type === "kw" &&
                part.tokens[0]?.value === "else")
                return;
            if (!Number.isFinite(part.endLine))
                return;
            incomplete.add(part.endLine);
            const start = Number.isFinite(part.startLine)
                ? part.startLine
                : part.endLine;
            for (let i = start; i <= part.endLine; i++) {
                if (invalid.has(i))
                    invalid.delete(i);
                if (errors.has(i))
                    errors.delete(i);
                if (errorKinds.has(i))
                    errorKinds.delete(i);
                if (info.has(i))
                    info.delete(i);
            }
        });
        incomplete.forEach((idx) => {
            if (invalid.has(idx))
                invalid.delete(idx);
            if (errors.has(idx))
                errors.delete(idx);
            if (errorKinds.has(idx))
                errorKinds.delete(idx);
        });
        ifBlocks.errors.forEach((message, line) => {
            invalid.add(line);
            errors.set(line, message);
            errorKinds.set(line, "compile");
        });
        ifBlocks.incomplete.forEach((line) => {
            incomplete.add(line);
            if (invalid.has(line))
                invalid.delete(line);
            if (errors.has(line))
                errors.delete(line);
            if (errorKinds.has(line))
                errorKinds.delete(line);
            if (info.has(line))
                info.delete(line);
        });
        whileBlocks.errors.forEach((message, line) => {
            invalid.add(line);
            errors.set(line, message);
            errorKinds.set(line, "compile");
        });
        whileBlocks.incomplete.forEach((line) => {
            incomplete.add(line);
            if (invalid.has(line))
                invalid.delete(line);
            if (errors.has(line))
                errors.delete(line);
            if (errorKinds.has(line))
                errorKinds.delete(line);
            if (info.has(line))
                info.delete(line);
        });
        return { invalid, incomplete, errors, errorKinds, info };
    }
    function executeProgramParts(parts, opts) {
        let state = [];
        const declared = new Set();
        const scopes = [new Set()];
        const ifBlocks = buildIfStatementMap(parts);
        const whileBlocks = buildWhileStatementMap(parts);
        const ifDecisions = new Map();
        const elseLookup = new Map();
        const whileStack = [];
        ifBlocks.map.forEach((block) => {
            if (block.elseIndex != null) {
                elseLookup.set(block.elseIndex, block);
            }
        });
        const continueCompletedWhileLoops = (nextIndex) => {
            let index = nextIndex;
            let guard = 0;
            while (whileStack.length > 0 && guard < parts.length + 5) {
                const active = whileStack[whileStack.length - 1];
                if (!active || index !== active.afterIndex)
                    break;
                whileStack.pop();
                index = active.headerIndex;
                guard += 1;
            }
            return index;
        };
        const elseEntryForIfBlock = (block) => {
            if (block.elseOpenIndex == null)
                return null;
            let nextIndex = block.elseOpenIndex;
            let pushScope = false;
            if (block.elseTarget != null && block.elseTarget !== block.elseOpenIndex) {
                const elseOpenPart = parts[block.elseOpenIndex];
                if (isBracePart(elseOpenPart, "{")) {
                    nextIndex = block.elseTarget;
                    pushScope = true;
                }
            }
            return { nextIndex, pushScope };
        };
        const maxExecutedParts = Math.max(10000, parts.length * 2000);
        const fallbackEndLine = parts.length > 0 && Number.isFinite(parts[parts.length - 1]?.endLine)
            ? parts[parts.length - 1].endLine
            : -1;
        const terminalBoundary = Math.max(0, fallbackEndLine + 1);
        const boundaryForProgramIndex = (index) => {
            if (!Number.isFinite(index) || index >= parts.length)
                return terminalBoundary;
            const safeIndex = Math.max(0, Math.floor(index));
            const line = parts[safeIndex]?.startLine;
            if (!Number.isFinite(line))
                return terminalBoundary;
            return Math.max(0, line);
        };
        let i = 0;
        let executedSteps = 0;
        let executedParts = 0;
        const isDeclLikeStatement = (parsed) => parsed.kind === "decl" || parsed.kind === "declAssign";
        const advanceTo = (nextIndex) => {
            const before = boundaryForProgramIndex(i);
            const after = boundaryForProgramIndex(nextIndex);
            if (after !== before)
                executedSteps += 1;
            return nextIndex;
        };
        const handleControlFlowStatement = (parsed, blockEndState) => {
            if (parsed.kind === "if") {
                const block = ifBlocks.map.get(i);
                if (!block)
                    return { kind: "error", errorKind: "compile" };
                const condition = evaluateCondition(parsed.expr, state);
                if ("error" in condition)
                    return { kind: "error", errorKind: condition.kind || "compile" };
                ifDecisions.set(block.headerIndex, condition.value);
                if (condition.value) {
                    i = advanceTo(i + 1);
                    return { kind: "continue" };
                }
                if (opts.stop !== null && opts.stop <= block.closeIndex) {
                    return { kind: "break" };
                }
                const elseEntry = elseEntryForIfBlock(block);
                if (elseEntry) {
                    if (elseEntry.pushScope)
                        scopes.push(new Set());
                    i = advanceTo(continueCompletedWhileLoops(elseEntry.nextIndex));
                    return { kind: "continue" };
                }
                i = advanceTo(continueCompletedWhileLoops(block.closeIndex + 1));
                return { kind: "continue" };
            }
            if (parsed.kind === "while") {
                const block = whileBlocks.map.get(i);
                if (!block)
                    return { kind: "error", errorKind: "compile" };
                const condition = evaluateCondition(parsed.expr, state);
                if ("error" in condition)
                    return { kind: "error", errorKind: condition.kind || "compile" };
                if (condition.value) {
                    whileStack.push(block);
                    i = advanceTo(block.openIndex);
                    return { kind: "continue" };
                }
                if (opts.stop !== null && opts.stop <= block.closeIndex) {
                    return { kind: "break" };
                }
                i = advanceTo(continueCompletedWhileLoops(block.afterIndex));
                return { kind: "continue" };
            }
            if (parsed.kind === "else") {
                const block = elseLookup.get(i);
                if (!block)
                    return { kind: "error", errorKind: "compile" };
                const decision = ifDecisions.get(block.headerIndex);
                if (decision == null)
                    return { kind: "error", errorKind: "compile" };
                if (decision) {
                    i = advanceTo(continueCompletedWhileLoops(block.afterIndex));
                    return { kind: "continue" };
                }
                const elseEntry = elseEntryForIfBlock(block);
                if (elseEntry) {
                    if (elseEntry.pushScope)
                        scopes.push(new Set());
                    i = advanceTo(continueCompletedWhileLoops(elseEntry.nextIndex));
                    return { kind: "continue" };
                }
                return { kind: "error", errorKind: "compile" };
            }
            if (parsed.kind === "blockStart") {
                scopes.push(new Set());
                i = advanceTo(continueCompletedWhileLoops(i + 1));
                return { kind: "continue" };
            }
            if (parsed.kind === "blockEnd") {
                const popped = popScope(scopes, declared, blockEndState);
                if (popped.error)
                    return { kind: "error", errorKind: "compile" };
                state = popped.state;
                i = advanceTo(continueCompletedWhileLoops(i + 1));
                return { kind: "continue" };
            }
            return { kind: "not-control" };
        };
        while (i < parts.length) {
            if (opts.stop !== null && i >= opts.stop)
                break;
            if (opts.stopSteps !== null && executedSteps >= opts.stopSteps)
                break;
            if (executedParts >= maxExecutedParts) {
                return { kind: "compile" };
            }
            executedParts += 1;
            const part = parts[i];
            if (!part.tokens.length) {
                i = continueCompletedWhileLoops(i + 1);
                continue;
            }
            if (opts.analyze) {
                const validation = validateStatement(part.tokens, state, declared, opts.alloc);
                if ("error" in validation) {
                    return { kind: validation.kind || "compile" };
                }
                const parsed = validation.parsed;
                const controlResult = handleControlFlowStatement(parsed, validation.next);
                if (controlResult.kind === "error") {
                    return { kind: controlResult.errorKind };
                }
                if (controlResult.kind === "break")
                    break;
                if (controlResult.kind === "continue")
                    continue;
                if (!part.hasSemicolon)
                    return { kind: "compile" };
                if (isDeclLikeStatement(parsed)) {
                    addDeclaredNames(scopes, declared, parsed.declaredNames);
                }
                state = validation.next;
                i = advanceTo(continueCompletedWhileLoops(i + 1));
                continue;
            }
            const parsed = parseStatementTokens(part.tokens);
            if (!parsed)
                return { kind: "compile" };
            const controlResult = handleControlFlowStatement(parsed, state);
            if (controlResult.kind === "error")
                return { kind: controlResult.errorKind };
            if (controlResult.kind === "break")
                break;
            if (controlResult.kind === "continue")
                continue;
            if (!part.hasSemicolon)
                return { kind: "compile" };
            if (isDeclLikeStatement(parsed)) {
                if (declared.has(parsed.name))
                    return { kind: "compile" };
            }
            const next = applyStatement(state, parsed, {
                alloc: opts.alloc,
                allowRedeclare: false,
            });
            if (!next)
                return { kind: "compile" };
            if (isDeclLikeStatement(parsed)) {
                addDeclaredNames(scopes, declared, parsed.declaredNames);
            }
            state = next;
            i = advanceTo(continueCompletedWhileLoops(i + 1));
        }
        return {
            kind: "ok",
            state,
            nextIndex: Math.max(0, Math.min(parts.length, i)),
            executedSteps,
        };
    }
    function applyProgram(text, opts = {}) {
        const tokens = tokenizeProgram(text);
        const parts = splitStatements(tokens);
        return applyProgramParts(parts, opts);
    }
    const resolveAlloc = (alloc) => alloc || ((type) => String(randAddr(type || "int")));
    const normalizeStop = (stop, max) => Number.isFinite(stop) && stop !== undefined
        ? Math.max(0, Math.min(max, Number(stop)))
        : null;
    const normalizeStopSteps = (stopSteps) => Number.isFinite(stopSteps) && stopSteps !== undefined
        ? Math.max(0, Number(stopSteps))
        : null;
    function applyProgramParts(parts, opts = {}) {
        const alloc = resolveAlloc(opts.alloc);
        const stop = normalizeStop(opts.stop, parts.length);
        const result = executeProgramParts(parts, {
            alloc,
            stop,
            stopSteps: null,
            analyze: false,
        });
        if (result.kind !== "ok")
            return null;
        return result.state;
    }
    function analyzeProgramParts(parts, opts = {}) {
        const alloc = resolveAlloc(opts.alloc);
        const stop = normalizeStop(opts.stop, parts.length);
        const result = executeProgramParts(parts, {
            alloc,
            stop,
            stopSteps: null,
            analyze: true,
        });
        if (result.kind !== "ok")
            return { kind: result.kind };
        return { kind: "ok", state: result.state };
    }
    function traceProgramParts(parts, opts = {}) {
        const alloc = resolveAlloc(opts.alloc);
        const stopSteps = normalizeStopSteps(opts.stopSteps);
        const result = executeProgramParts(parts, {
            alloc,
            stop: null,
            stopSteps,
            analyze: false,
        });
        if (result.kind !== "ok")
            return null;
        return {
            state: result.state,
            nextIndex: result.nextIndex,
            executedSteps: result.executedSteps,
        };
    }
    return {
        tokenizeProgram,
        splitStatements,
        parseStatements,
        buildStatementMap,
        buildIfStatementMap,
        buildWhileStatementMap,
        statementRangeForLine,
        getStatementContext,
        evaluateCondition,
        evaluateExpressionText,
        classifyLineStatuses,
        findMissingSemicolonLines,
        applyProgramParts,
        analyzeProgramParts,
        traceProgramParts,
        applyProgram,
    };
}
