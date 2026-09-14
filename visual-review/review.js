/* Screenshot review helper. Loaded from disk (file://); no modules. */
(function (root) {
  function selectedNames(items) {
    var out = [];
    for (var i = 0; i < items.length; i++) {
      if (items[i].checked) out.push(items[i].name);
    }
    return out;
  }

  function disapprovalClipboard(names) {
    if (!names || names.length === 0) return "";
    var lines = ["DISAPPROVED:"];
    for (var i = 0; i < names.length; i++) {
      lines.push("- " + names[i]);
    }
    return lines.join("\n");
  }

  root.selectedNames = selectedNames;
  root.disapprovalClipboard = disapprovalClipboard;
})(typeof globalThis !== "undefined" ? globalThis : this);
