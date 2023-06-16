document.addEventListener("DOMContentLoaded", updatePrefix);

async function updatePrefix() {
    const serverConfig = await (await fetch("/serverconfig.json")).json();
    if (serverConfig["public_prefix"] !== null) {
        const prefixEl = document.getElementById("ipv6-prefix");
        prefixEl.innerText = serverConfig["public_prefix"];
    } else {
        console.log("No public prefix was specified!");
    }
}
