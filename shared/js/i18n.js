/* SPDX-License-Identifier: MPL-2.0 */
// Translated UI (#379): the message catalogs of shared/i18n/ for the desktop
// app (and any page of the web UI that renders in JavaScript). The server
// reads the same files (src/i18n.rs, the twin of this module); background in
// docs/src/research/translated-ui.md.
//
// A message is a sentence with named placeholders ({count}, {item}) or an
// object of CLDR plural categories chosen by `count`. A message missing in
// the language falls back to English, an unknown ID shows the ID. Values are
// inserted in one pass after the lookup – a value with "{x}" in it is never
// expanded – and callers put the result into the page as text.

export var LANGUAGES = ["en", "de"];

var catalogs = { en: {} };
var current = "en";

/** The shipped language for a tag (`de-AT` → `de`), or null. */
export function supported(tag) {
  tag = String(tag || "").trim().toLowerCase();
  var base = tag.split(/[-_]/)[0];
  if (LANGUAGES.indexOf(tag) !== -1) return tag;
  return LANGUAGES.indexOf(base) !== -1 ? base : null;
}

/** The language for preferred tags in order (navigator.languages): each
 *  exactly, then by its base language; English when none fits. */
export function negotiate(tags) {
  for (var i = 0; i < (tags || []).length; i++) {
    var found = supported(tags[i]);
    if (found) return found;
  }
  return "en";
}

/** Inserts `params` into `text` in one pass. */
export function fill(text, params) {
  params = params || {};
  return String(text).replace(/\{([A-Za-z0-9_]+)\}/g, function (all, name) {
    return Object.prototype.hasOwnProperty.call(params, name) ? String(params[name]) : all;
  });
}

function textOf(lang, id, count) {
  var message = (catalogs[lang] || {})[id];
  if (typeof message === "string") return message;
  if (message && typeof message === "object") {
    var category = new Intl.PluralRules(lang).select(typeof count === "number" ? count : 0);
    return message[category] || message.other || null;
  }
  return null;
}

/** Message `id` in the current language with `params` filled in. */
export function t(id, params) {
  var count = params && typeof params.count === "number" ? params.count : undefined;
  var text = textOf(current, id, count);
  if (text === null) text = textOf("en", id, count);
  return fill(text === null ? id : text, params);
}

/** The current language. */
export function currentLanguage() { return current; }

/** Loads English and `lang` from `base` (`/static/shared/i18n/`) and makes
 *  `lang` current; a catalog that cannot be loaded leaves English. */
export function loadLanguage(lang, base) {
  base = base || "/static/shared/i18n/";
  lang = supported(lang) || "en";
  function load(l) {
    if (catalogs[l] && Object.keys(catalogs[l]).length) return Promise.resolve();
    return fetch(base + l + ".json").then(function (r) { return r.ok ? r.json() : {}; })
      .then(function (c) { catalogs[l] = c || {}; }, function () { catalogs[l] = {}; });
  }
  return load("en").then(function () { return lang === "en" ? null : load(lang); }).then(function () {
    current = lang;
    if (typeof document !== "undefined") document.documentElement.setAttribute("lang", lang);
    return lang;
  });
}

/** Sets a catalog directly (tests). */
export function useCatalogs(map, lang) {
  catalogs = map;
  current = lang || "en";
}
