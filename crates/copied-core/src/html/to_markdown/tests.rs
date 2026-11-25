use super::*;

#[test]
fn converts_headings_paragraphs_and_breaks() {
    let html = "<h1>One</h1><h2>Two</h2><h3>Three</h3><h4>Four</h4><h5>Five</h5><h6>Six</h6>
        <div><p> a   &amp; <span> b</span> <br> c&nbsp; d </p></div><hr><div>Last</div>";
    assert_eq!(
        html_to_markdown(html),
        "# One\n\n## Two\n\n### Three\n\n#### Four\n\n##### Five\n\n###### Six\n\na & b\\\nc d\n\n***\n\nLast"
    );
}

#[test]
fn converts_formatting_and_keeps_whitespace_outside_markers() {
    let html = "<p>A<strong> bold </strong>and <b>strong</b>, <em>em</em> or <i>italic</i>.
        <s>s</s>, <del>del</del>, <strike>strike</strike>. <code>x`y</code></p>";
    assert_eq!(
        html_to_markdown(html),
        "A **bold** and **strong**, *em* or *italic*. ~~s~~, ~~del~~, ~~strike~~. ``x`y``"
    );
    assert_eq!(
        html_to_markdown(
            r#"<b style="font-weight: normal"><p><span style="font-weight:700">Docs</span>
            <span style="font-style: italic; text-decoration: line-through">edit</span></p></b>"#
        ),
        "**Docs** *~~edit~~*"
    );
    assert_eq!(html_to_markdown("<b><strong>Word</strong></b>"), "**Word**");
}

#[test]
fn retains_allowed_destinations_and_escapes_literal_markdown() {
    let html = r#"<a href="https://example.com/a(b)?x=1&amp;y=2">web</a>
        <a href="http://example.com">http</a> <a href="mailto:hi@example.com">email</a>
        <a href="jav&#x09;ascript:alert(1)"><b>unsafe</b></a>
        <a href="DATA:text/plain,hello">data</a> <a href="/relative">relative</a> <a>bare</a>
        <p>[label] &lt;b> x*y &amp;copy;</p>"#;
    let markdown = html_to_markdown(html);
    assert!(
        markdown.starts_with(concat!(
            "[web](https://example.com/a\\(b\\)?x=1\\&y=2) ",
            "[http](http://example.com) [email](mailto:hi@example.com) ",
            "**unsafe** data relative bare\n\n"
        )),
        "{markdown}"
    );
    assert!(markdown.contains("\\[label]"), "{markdown}");
    assert!(markdown.contains("\\<b>"), "{markdown}");
    assert!(markdown.contains("\\&copy;"), "{markdown}");
}

#[test]
fn keeps_code_layout_and_uses_a_longer_fence() {
    let html = "<pre class='language-txt'><code class='x language-rust'>fn main() {\n    ```\n}\n</code></pre>
        <pre class='language-sh'>echo &lt;hi&gt;<br>  finish</pre>";
    assert_eq!(
        html_to_markdown(html),
        "````rust\nfn main() {\n    ```\n}\n````\n\n```sh\necho <hi>\n  finish\n```"
    );
    assert_eq!(
        html_to_markdown("<pre>one\n\n\n</pre>"),
        "```\none\n\n\n```"
    );
    assert_eq!(html_to_markdown("<pre><code>\n</code></pre>"), "```\n\n```");
}

#[test]
fn keeps_editor_div_lines_and_indentation() {
    let html = concat!(
        r#"<div class="language-ts" style="color: #000; white-space: pre; font-family: Consolas">"#,
        r#"<div><span style="color: blue">const</span><span> x = {</span></div>"#,
        r#"<div><span style="color: green">    ```</span></div>"#,
        "<div><br></div><div>}</div></div>",
    );
    assert_eq!(
        html_to_markdown(html),
        "````ts\nconst x = {\n    ```\n\n}\n````"
    );
    for value in ["PRE-WRAP", "break-spaces !important"] {
        assert_eq!(
            html_to_markdown(&format!(
                "<div style='WHITE-SPACE: {value}'><p>\tone<br>  two</p><p>three</p></div>"
            )),
            "```\n\tone\n  two\nthree\n```"
        );
    }
    assert_eq!(
        html_to_markdown(
            "<div style='/* editor */ white-space: pre !important; white-space: normal'><div> a  </div><div></div></div>"
        ),
        "```\n a  \n\n```"
    );
}

#[test]
fn leaves_styled_prose_as_prose() {
    for style in [
        "color: red; white-space: normal",
        "white-space: pre; white-space: normal",
        "--white-space: pre",
        "font-family: &quot;foo; white-space: pre; bar&quot;",
        "background: url(data:text/plain;white-space:pre;code)",
        "/* ; white-space: pre; */ color: red",
    ] {
        assert_eq!(
            html_to_markdown(&format!("<div style='{style}'> a   b </div>")),
            "a b",
            "{style}"
        );
    }
}

#[test]
fn resets_inherited_inline_marks_and_resumes_them_afterward() {
    let html = concat!(
        "<strong>bold <span style='font-weight:normal'>plain</span> bold2</strong>",
        "<p><em>italic <span style='font-style:normal'>plain</span></em></p>",
        "<p><del>gone<span style='text-decoration:none'> kept </span>gone2</del></p>",
        "<p><em><strong>both <i style='font-weight:400'>just italic</i> both</strong></em></p>",
    );
    assert_eq!(
        html_to_markdown(html),
        "**bold** plain **bold2**\n\n*italic* plain\n\n~~gone~~ kept ~~gone2~~\n\n***both** just italic **both***"
    );
    assert_eq!(
        html_to_markdown(concat!(
            "<strong>before <a href='https://example.com'>bold ",
            "<span style='font-weight: 400'>plain</span></a> after</strong>",
        )),
        "**before** [**bold** plain](https://example.com) **after**"
    );
}

#[test]
fn preserves_tex_annotations_and_readable_mathml() {
    let html = concat!(
        r#"<p>Euler: <math><semantics><mi>ignored</mi><annotation encoding="application/x-tex">e^{i\pi}+1=0</annotation></semantics></math>.</p>"#,
        r#"<math display="block"><semantics><mn>0</mn><annotation encoding="text/x-latex">\frac{1}{2}</annotation></semantics></math>"#,
        r#"<p><math><semantics><mrow><mi>x</mi><mo>+</mo><mn>1</mn></mrow><annotation encoding="text/plain">noise</annotation><annotation-xml encoding="application/xml"><extra>noise</extra></annotation-xml></semantics></math></p>"#,
    );
    assert_eq!(
        html_to_markdown(html),
        "Euler: $e^{i\\pi}+1=0$.\n\n$$\\frac{1}{2}$$\n\nx+1"
    );
    assert_eq!(
        html_to_markdown("<math><annotation encoding='TeX'>y^2</annotation></math>"),
        "$y^2$"
    );
    assert_eq!(
        html_to_markdown("<math><annotation encoding='TeX'>x&lt;img src=x&gt;</annotation></math>"),
        "$x&#x3C;img src=x>$"
    );
}

#[test]
fn converts_nested_lists_tasks_and_quotes() {
    let html = "<blockquote><p>Quoted</p><blockquote>Nested</blockquote></blockquote>
        <ol start='3'><li><p>First</p><ul><li><input type='checkbox' checked> Done</li>
        <li><p><label><input type='CHECKBOX'> Next</label></p></li></ul></li><li>Last</li></ol>";
    assert_eq!(
        html_to_markdown(html),
        "> Quoted\n>\n> > Nested\n\n3. First\n   - [x] Done\n   - [ ] Next\n4. Last"
    );
}

#[test]
fn converts_tables_with_inline_formatting_and_missing_cells() {
    let html = "<table><thead><tr><th>Name</th><th align='right'>Value</th></tr></thead>
        <tbody><tr><td><b>A | B</b><br>next</td><td><code>2|3</code></td></tr>
        <tr><td>Only one</td></tr></tbody></table>";
    assert_eq!(
        html_to_markdown(html),
        "| Name | Value |\n| --- | ---: |\n| **A \\| B**<br>next | `2\\|3` |\n| Only one |"
    );
}

#[test]
fn converts_headerless_tables_and_css_alignment() {
    assert_eq!(
        html_to_markdown(concat!(
            "<table><tr><td align='left' style='text-align: CENTER'>A<br>B</td>",
            "<td style='text-align: right'>C</td></tr><tr><td>D<td>E</table>",
        )),
        "| | |\n| :---: | ---: |\n| A<br>B | C |\n| D | E |"
    );
    assert_eq!(
        html_to_markdown("<table><tr><td>first</td></tr><tr><td>second</td></tr></table>"),
        "first\n\nsecond"
    );
    assert_eq!(
        html_to_markdown("<table><thead><tr><td>A<td>B</thead><tr><td>C<td>D</table>"),
        "| A | B |\n| --- | --- |\n| C | D |"
    );
    assert_eq!(
        html_to_markdown(concat!(
            "<table><tr><th style='text-align: left'>Value</th></tr>",
            "<tr><td><a href='https://example.com'>A<br>B</a><br><br>",
            "<math><annotation encoding='TeX'>|x|</annotation></math></td></tr></table>",
        )),
        "| Value |\n| :--- |\n| [A<br>B](https://example.com)<br><br>$\\|x\\|$ |"
    );
}

#[test]
fn keeps_cells_in_column_order_when_table_cells_span() {
    assert_eq!(
        html_to_markdown(concat!(
            "<table><tr><th colspan='2'>Group</th><th>Value</th></tr>",
            "<tr><td rowspan='2'>A</td><td>B</td><td>C</td></tr>",
            "<tr><td>D</td><td>E</td></tr></table>",
        )),
        "| Group | | Value |\n| --- | --- | --- |\n| A | B | C |\n| | D | E |"
    );
    assert_eq!(
        html_to_markdown("<table><tr><td colspan='1000'>Wide<td>Next<tr><td>Another</table>"),
        "Wide | Next\n\nAnother"
    );
}

#[test]
fn strips_entire_inert_subtrees_and_html_comments() {
    let html = "<!doctype html><html><head><title>Hidden</title><script>bad()</script>
        <noscript>head alternative</noscript></head><body><!--StartFragment--><p>Visible</p>
        <script><b>hidden</b></script><style>body { color: red }</style>
        <noscript><p>disabled</p></noscript><template><p>template</p></template>
        <pre>keep<script>hide</script><br>going</pre><!--EndFragment--></body></html>";
    assert_eq!(html_to_markdown(html), "Visible\n\n```\nkeep\ngoing\n```");
}

#[test]
fn recovers_malformed_input_and_bounds_large_or_deep_html() {
    for empty in [
        "",
        " \t\n",
        "<!-- empty -->",
        "<p> <br></p>",
        "<div><script>never",
    ] {
        assert!(html_to_markdown(empty).is_empty(), "{empty}");
    }
    assert_eq!(html_to_markdown("<div><p>Keep <b>going"), "Keep **going**");
    assert_eq!(html_to_markdown(&"é".repeat(MAX_INPUT_BYTES / 2 + 1)), "");
    let shallow = "<div>".repeat(MAX_HTML_DEPTH / 2) + "visible";
    assert_eq!(html_to_markdown(&shallow), "visible");
    let deep = "visible".to_owned() + &"<div>".repeat(MAX_HTML_DEPTH) + "too deep";
    assert_eq!(html_to_markdown(&deep), "visible");
    let hostile = "<div>".repeat(MAX_HTML_DEPTH * 30) + "discarded";
    assert_eq!(html_to_markdown(&hostile), "");
}

#[test]
fn bounds_indentation_amplification() {
    let quotes = "<blockquote>".repeat(80);
    let html = format!("{quotes}<pre>{}</pre>", "x\n".repeat(60_000));
    assert_eq!(html_to_markdown(&html), "");
}
