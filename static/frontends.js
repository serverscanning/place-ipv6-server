const ALL_FRONTENDS = {
    // Name: Directory
    "Blue": "blue",
    "TimR": "timr",
    "Ziad": "ziad"
};

function getCurrentFrontedDirectory() {
    return location.pathname.substring(1).split("/")[0];
}

document.addEventListener("DOMContentLoaded", updateOtherFrontendsList);

function updateOtherFrontendsList() {
    const otherFrontendsListEl = document.getElementById("other-frontends-list");

    const currentFrontedDirectory = getCurrentFrontedDirectory();
    for (const frontendName in ALL_FRONTENDS) {
        const frontendDirectory = ALL_FRONTENDS[frontendName];
        if (frontendDirectory === currentFrontedDirectory) continue;

        let listItemEl = document.createElement("li");
        let linkEl = document.createElement("a");
        linkEl.href = "/" + frontendDirectory + "/";
        linkEl.innerText = frontendName;

        listItemEl.appendChild(linkEl);
        otherFrontendsListEl.appendChild(listItemEl);
    }
}
