use std::collections::BTreeMap;

use quantic_engine::{html, js_runtime::DomMutation, BoaRuntime, JsRuntime};

fn assert_script_output(script: &str, expected: &str) {
    let document = html::parse("<p id='out'>pending</p>");
    let mut runtime = BoaRuntime::new(&document, &BTreeMap::new()).unwrap();
    runtime.eval_script(script).unwrap();
    let effects = runtime.drain_effects().unwrap();
    let actual = effects
        .mutations
        .iter()
        .rev()
        .find_map(|mutation| match mutation {
            DomMutation::SetText { value, .. } => Some(value.as_str()),
            _ => None,
        });
    assert_eq!(actual, Some(expected));
}

#[test]
fn existing_search_params_and_iterators_follow_url_replacement() {
    assert_script_output(
        "const url = new URL('https://example.test/?a=1&b=2&c=3');
         const params = url.searchParams;
         const entries = params.entries();
         const first = entries.next().value.join(':');
         url.search = '?x=7&y=8&z=9';
         const second = entries.next().value.join(':');
         const same = params === url.searchParams;
         url.href = 'https://example.test/next?p=10';
         params.append('q', '11');
         document.getElementById('out').textContent =
           [first, second, same, params.get('p'), url.search].join('|');",
        "a:1|y:8|true|10|?p=10&q=11",
    );
}

#[test]
fn entries_observe_deletions_during_iteration() {
    assert_script_output(
        "const params = new URLSearchParams('a=1&b=2&c=3');
         const seen = [];
         for (const [key, value] of params) {
           if (key === 'a') params.delete('b');
           seen.push(key + ':' + value);
         }
         document.getElementById('out').textContent = seen.join(',');",
        "a:1,c:3",
    );
}

#[test]
fn keys_values_and_for_each_observe_live_mutations() {
    assert_script_output(
        "const keysParams = new URLSearchParams('a=1&b=2&c=3');
         const keys = keysParams.keys();
         const firstKey = keys.next().value;
         keysParams.delete('b');
         const nextKey = keys.next().value;
         const valuesParams = new URLSearchParams('a=1&b=2');
         const values = valuesParams.values();
         const firstValue = values.next().value;
         valuesParams.append('c', '3');
         const remainingValues = [...values].join(',');
         const eachParams = new URLSearchParams('a=1&b=2&c=3');
         const seen = [];
         eachParams.forEach((value, key, owner) => {
           if (key === 'a') owner.delete('b');
           seen.push(key + ':' + value);
         });
         document.getElementById('out').textContent =
           [firstKey, nextKey, firstValue, remainingValues, seen.join(',')].join('|');",
        "a|c|1|2,3|a:1,c:3",
    );
}

#[test]
fn opaque_url_query_changes_preserve_the_path_without_an_authority() {
    assert_script_output(
        "const url = new URL('data:space    ?test#fragment');
         url.searchParams.delete('test');
         document.getElementById('out').textContent =
           [url.pathname, url.search, url.href, url.origin].join('|');",
        "space   %20||data:space   %20#fragment|null",
    );
}

#[test]
fn parameter_iterators_can_be_reused_after_an_early_loop_exit() {
    assert_script_output(
        "const params = new URLSearchParams('a=1&b=2');
         const iterators = [params.entries(), params.keys(), params.values()];
         const resumed = iterators.map(iterator => {
           for (const value of iterator) break;
           return iterator.next().value;
         });
         document.getElementById('out').textContent = JSON.stringify(resumed);",
        "[[\"b\",\"2\"],\"b\",\"2\"]",
    );
}

#[test]
fn exhausted_parameter_iterators_observe_later_appends() {
    assert_script_output(
        "const params = new URLSearchParams();
         const iterators = [params.entries(), params.keys(), params.values()];
         const initiallyDone = iterators.every(iterator => iterator.next().done);
         params.append('late', '9');
         const resumed = iterators.map(iterator => JSON.stringify(iterator.next().value));
         document.getElementById('out').textContent =
           [initiallyDone, ...resumed].join('|');",
        "true|[\"late\",\"9\"]|\"late\"|\"9\"",
    );
}

#[test]
fn empty_parameter_for_each_validates_its_callback() {
    assert_script_output(
        "let rejected = false;
         try { new URLSearchParams().forEach(null); }
         catch (error) { rejected = error instanceof TypeError; }
         document.getElementById('out').textContent = String(rejected);",
        "true",
    );
}
