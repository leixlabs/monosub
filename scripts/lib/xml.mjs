// Minimal XML editing for the generated Package.appxmanifest.
//
// ## Why not a real DOM
//
// Node has no XML parser, and this file exists because the Windows build has to
// rewrite five specific things in a manifest that `winapp manifest generate`
// produced: DisplayName (twice, on two different elements),
// PublisherDisplayName, the Identity processor architecture, the Application
// executable, and an app execution alias that must be nested inside
// <Application><Extensions>.
//
// A regex per edit is what the bash version of this build used, and it is how
// the Start-menu name shipped lowercase once: the substitution matched
// <Properties><DisplayName> and silently missed <VisualElements DisplayName="">.
// So instead of regexes, the file is tokenised into real tags — attribute values
// in quotes, comments and CDATA skipped — and edits are spliced by offset. A
// tag that is not found raises instead of silently doing nothing.
//
// ## Why not write the manifest from scratch
//
// `winapp manifest update-assets` generates and registers the Store tile PNGs
// from the app icon and rewrites the manifest to reference them. Authoring the
// file here would mean re-implementing the asset pipeline.

// The `/` in the self-closing group is escaped because this is a regex literal;
// unescaped it terminates the literal at that point.
// The name group allows ':': the manifest mixes the foundation namespace with
// uap/uap3/desktop, and a name pattern without the colon would parse
// `<uap3:Extension ...>` as an element literally named `uap3`.
const TAG = /<([!?/]?)([A-Za-z_][\w.:-]*)((?:\s[^<>]*)?)([/]?)>/g;

/**
 * Every element tag in `xml`.
 *
 * Returns `{ localName, start, end, tagEnd, selfClosing, attrs, attrsText }`
 * where `start..end` is the whole `<...>` and `tagEnd` is where its children
 * begin. Prefixes are kept in `rawName` and stripped in `localName`, because the
 * manifest mixes the foundation namespace with `uap`/`uap3`/`desktop`.
 */
export function scanTags(xml) {
    const tags = [];
    let inComment = false;
    let inCdata = false;
    let depth = 0;

    TAG.lastIndex = 0;
    let match;
    while ((match = TAG.exec(xml)) !== null) {
        const [full, marker, rawName, attrsText, selfClose] = match;
        const start = match.index;

        if (inComment) {
            if (full.startsWith('<!--')) continue;
            if (full === '-->') inComment = false;
            continue;
        }
        if (inCdata) {
            if (full === ']]>') inCdata = false;
            continue;
        }
        if (marker === '!') {
            if (full === '<!--') inComment = true;
            else if (full.startsWith('<![CDATA[')) inCdata = true;
            continue;
        }
        if (marker === '?') continue; // XML declaration / processing instruction

        const localName = rawName.includes(':') ? rawName.slice(rawName.indexOf(':') + 1) : rawName;
        const selfClosing = selfClose === '/' || full.endsWith('/>');

        tags.push({
            rawName,
            localName,
            start,
            end: start + full.length,
            tagEnd: start + full.length,
            selfClosing,
            closing: marker === '/',
            attrsText,
            depth,
        });

        if (!selfClosing) depth += marker === '/' ? -1 : 1;
    }
    return tags;
}

/** `{ start, end }` of the children of the element at `tag`, scanning forward. */
function childrenRange(xml, tag) {
    if (tag.selfClosing) return { start: tag.tagEnd, end: tag.tagEnd, selfClosing: true };

    // Matched by offset, not identity: `findChild` hands back a re-offset copy of
    // the tag it found, and an identity lookup in a fresh scan would never match it.
    const tags = scanTags(xml);
    const index = tags.findIndex((candidate) => candidate.start === tag.start);
    if (index < 0) throw new Error('tag not part of the document it came from');

    let depth = 0;
    for (let i = index; i < tags.length; i++) {
        const current = tags[i];
        if (i === index) {
            depth = 1;
            continue;
        }
        if (current.closing) depth -= 1;
        else if (!current.selfClosing) depth += 1;
        if (depth === 0) return { start: tag.tagEnd, end: current.start };
    }
    throw new Error(`unbalanced XML around <${tag.rawName}>`);
}

function parseAttrs(attrsText) {
    const attrs = new Map();
    const re = /([A-Za-z_][\w:.-]*)\s*=\s*"([^"]*)"/g;
    let match;
    while ((match = re.exec(attrsText)) !== null) {
        attrs.set(match[1], match[2]);
    }
    return attrs;
}

export function getAttr(tag, name) {
    return parseAttrs(tag.attrsText).get(name) ?? null;
}

/** The first element with `localName` inside `parent`'s children, or null. */
export function findChild(xml, parent, localName) {
    const range = childrenRange(xml, parent);
    if (range.selfClosing) return null;
    const inner = xml.slice(range.start, range.end);
    const innerTags = scanTags(inner).filter((tag) => !tag.closing);
    const found = innerTags.find((tag) => tag.localName === localName);
    if (!found) return null;
    // Re-offset into the parent document.
    return { ...found, start: found.start + range.start, end: found.end + range.start, tagEnd: found.tagEnd + range.start };
}

/** The root element of the document. */
export function root(xml, expectedLocalName) {
    const tags = scanTags(xml).filter((tag) => !tag.closing);
    if (tags.length === 0) throw new Error('document has no elements');
    const node = tags[0];
    if (expectedLocalName && node.localName !== expectedLocalName) {
        throw new Error(`expected a <${expectedLocalName}> document, found <${node.rawName}>`);
    }
    return node;
}

/** Set `name="value"` on `tag`, inserting it before the closing bracket. */
export function setAttr(xml, tag, name, value) {
    const attrs = parseAttrs(tag.attrsText);
    const quote = '"';

    if (attrs.has(name)) {
        const re = new RegExp(`\\s${escapeRegExp(name)}\\s*=\\s*"[^"]*"`);
        const slice = xml.slice(tag.start, tag.end);
        const replaced = slice.replace(re, ` ${name}=${quote}${escapeAttr(value)}${quote}`);
        return splice(xml, tag.start, tag.end, replaced);
    }

    const slice = xml.slice(tag.start, tag.end);
    const at = tag.selfClosing ? slice.length - 2 : slice.length - 1;
    const injected = `${slice.slice(0, at)} ${name}=${quote}${escapeAttr(value)}${quote}${slice.slice(at)}`;
    return splice(xml, tag.start, tag.end, injected);
}

/** Read an attribute of the root element, e.g. `IgnorableNamespaces`. */
export function getRootAttr(xml, name) {
    return getAttr(root(xml), name);
}

/** Set an attribute on the root element (namespace declarations included). */
export function setRootAttr(xml, name, value) {
    return setAttr(xml, root(xml), name, value);
}

/**
 * Set the text of `<localName>` inside `parent`, creating the element when it is
 * missing. The old scripts relied on the generator emitting one; it does not
 * always, and a manifest without DisplayName packs fine and fails certification.
 */
export function setChildText(xml, parent, localName, text) {
    const existing = findChild(xml, parent, localName);
    if (existing) {
        const range = childrenRange(xml, existing);
        return splice(xml, range.start, range.end, escapeText(text));
    }

    const range = childrenRange(xml, parent);
    const indent = detectIndent(xml, parent);
    const element = `<${localName}>${escapeText(text)}</${localName}>`;
    return splice(xml, range.start, range.start, `\n${indent}  ${element}`);
}

/** Text inside the first `localName` child of `parent`, or '' when absent. */
export function childText(xml, parent, localName) {
    const child = findChild(xml, parent, localName);
    if (!child) return '';
    const range = childrenRange(xml, child);
    return xml.slice(range.start, range.end).trim();
}

/**
 * Append `childXml` inside `parent`'s `<Extensions>` (or directly inside
 * `parent` when `wrapper` is null).
 *
 * Used for the execution alias, which the appx schema only accepts inside
 * <Application><Extensions> — an extension that is a direct child of
 * <Application> packs and then fails validation with C00CE014.
 */
export function appendChild(xml, parent, childXml, wrapperLocalName = 'Extensions') {
    const indent = detectIndent(xml, parent);
    const element = `\n${indent}  ${childXml}`;

    if (!wrapperLocalName) {
        const range = childrenRange(xml, parent);
        return splice(xml, range.end, range.end, element);
    }

    const wrapper = findChild(xml, parent, wrapperLocalName);
    if (!wrapper) {
        const range = childrenRange(xml, parent);
        const wrapped = `\n${indent}  <${wrapperLocalName}>${element.replace(/\n/g, `\n  `)}\n${indent}  </${wrapperLocalName}>`;
        return splice(xml, range.end, range.end, wrapped);
    }

    const range = childrenRange(xml, wrapper);
    return splice(xml, range.end, range.end, element);
}

/** Replace `from`..`to` with `text`, keeping the rest of the document byte-identical. */
function splice(xml, from, to, text) {
    return xml.slice(0, from) + text + xml.slice(to);
}

function detectIndent(xml, tag) {
    const lineStart = xml.lastIndexOf('\n', tag.start) + 1;
    const prefix = xml.slice(lineStart, tag.start);
    return prefix.match(/^\s*/)?.[0] ?? '';
}

function escapeRegExp(value) {
    return value.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
}

function escapeAttr(value) {
    return String(value).replace(/&/g, '&amp;').replace(/"/g, '&quot;').replace(/</g, '&lt;');
}

function escapeText(value) {
    return String(value).replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
}
