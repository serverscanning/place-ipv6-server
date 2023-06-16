document.addEventListener("DOMContentLoaded", updatePrefix);

function updatePrefix() {
    const prefixEl = document.getElementById("ipv6-prefix");
    prefixEl.innerText = "2a01:4f8:c012:f8e6";
}
