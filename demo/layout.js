// yatare demo — naive jqLite on a simplified virtual DOM, 2000s style.
//
// Layout/offset math: reposition the banner, push the sidebar down, pack the
// result for the host harness. The harness hydrates manifest slots from its
// own DOM, runs this compiled bytecode, and writes the results back.
//
// Subset notes: i32 math only, strings are compile-time (selectors / css
// props), functions are inlined, `&&`/`||` return 1/0. Edge cases: none.

function clamp(v, hi) {
    if (v > hi) {
        return hi;
    }
    if (v < 0) {
        return 0;
    }
    return v;
}

var vw = 960;
var left = $("#banner").offset().left + 20;
var width = $("#banner").width() + 40;

if (!(left < 0) && left + width > vw) {
    left = vw - width;
}
left = clamp(left, vw);
$("#banner").css("left", left);

var top = $("#sidebar").offset().top;
while (top < 16) {
    top += 16;
}
$("#sidebar").css("top", top);

return left * 1000 + top;
