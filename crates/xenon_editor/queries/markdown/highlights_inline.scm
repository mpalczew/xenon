; Markdown inline highlights (tree-sitter-md + Xenon captures).
; Provenance: nvim-treesitter / tree-sitter-md stock, extended for GFM.

[
  (code_span)
  (link_title)
] @text.literal

[
  (emphasis_delimiter)
  (code_span_delimiter)
] @punctuation.delimiter

(emphasis) @text.emphasis

(strong_emphasis) @text.strong

(strikethrough) @text.strike

[
  (link_destination)
  (uri_autolink)
  (email_autolink)
] @text.uri

[
  (link_label)
  (link_text)
  (image_description)
] @text.reference

[
  (backslash_escape)
  (hard_line_break)
] @string.escape

(image
  [
    "!"
    "["
    "]"
    "("
    ")"
  ] @punctuation.delimiter)

(inline_link
  [
    "["
    "]"
    "("
    ")"
  ] @punctuation.delimiter)

(full_reference_link
  [
    "["
    "]"
  ] @punctuation.delimiter)

(collapsed_reference_link
  [
    "["
    "]"
  ] @punctuation.delimiter)

(shortcut_link
  [
    "["
    "]"
  ] @punctuation.delimiter)
