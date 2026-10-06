test(() => {
  const params = new URLSearchParams("a=1&b=2");
  params.set("a", "7");
  assert_equals(params.get("a"), "7");
  assert_equals([...params.keys()].join(","), "a,b");
}, "URLSearchParams live surface");

test(() => {
  const url = new URL("https://example.test/path?q=1");
  url.searchParams.append("mode", "gekko");
  assert_equals(url.search, "?q=1&mode=gekko");
  assert_equals(url.origin, "https://example.test");
}, "URL and searchParams stay linked");

test(() => {
  const bytes = new TextEncoder().encode("hé");
  assert_equals(bytes.length, 3);
}, "TextEncoder UTF-8");

test(() => {
  const target = document.createElement("div");
  let count = 0;
  target.addEventListener("gekko", () => count++);
  target.dispatchEvent(new Event("gekko"));
  assert_equals(count, 1);
}, "DOM event dispatch");

test(() => {
  const node = document.createElement("p");
  node.dataset.mode = "private";
  assert_equals(node.dataset.mode, "private");
}, "dataset reflection");
