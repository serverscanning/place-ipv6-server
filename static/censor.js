censorState = {
    isCensored: false,
    canvasEl: null,
    censorEl: null,
    windowResizeHandler: null,
    originalParentHref: null,
    resizeObserver: null,
}

function censorCanvas(description, { title = "Unsafe content ahead!", descriptionIsHtml = false, canvasEl = undefined } = {}) {
    if (localStorage.getItem("hide_censor_forever") === "true") {
        console.log("WARN: Not censoring canvas because the user has disabled censoring forever!");
        return;
    }
    if (censorState.isCensored) uncensorCanvas({ canvasEl });
    console.log("Censoring canvas...");


    if (!canvasEl)
        canvasEl = document.getElementById("canvas");
    if (canvasEl === null) {
        console.log("WARN: Failed to apply censoring!");
        return;
    }
    censorState.isCensored = true;
    censorState.canvasEl = canvasEl;

    if (canvasEl.parentElement.tagName.toLowerCase() === "a" && canvasEl.parentElement.hasAttribute("href")) {
        censorState.originalParentHref = canvasEl.parentElement.getAttribute("href");
        canvasEl.parentElement.removeAttribute("href");
    }

    const censorWrapperContainerEl = document.createElement("div");
    censorWrapperContainerEl.id = "censor";
    const censorContainerEl = document.createElement("div");

    canvasEl.parentElement.insertBefore(censorWrapperContainerEl, canvasEl);
    censorWrapperContainerEl.appendChild(censorContainerEl);
    censorState.censorEl = censorWrapperContainerEl;

    const censorTitle = document.createElement("h1");
    censorTitle.innerText = title;

    const censorText1 = document.createElement("p");
    if (descriptionIsHtml)
        censorText1.innerHTML = description;
    else
        censorText1.innerText = description;

    const censorText2 = document.createElement("p");
    censorText2.innerText = "If you're a minor, DO NOT click the button blow. The content could be nasty! This will disappear once the content is safe again.";

    const hideCensorButton = document.createElement("a");
    hideCensorButton.classList.add("button");
    hideCensorButton.innerText = "Show anyway";

    const hideCensorForeverContainer = document.createElement("div");
    hideCensorForeverContainer.id = "censor-hide-forever";
    const hideCensorForeverCheckbox = document.createElement("input")
    hideCensorForeverCheckbox.type = "checkbox";
    hideCensorForeverCheckbox.id = "censor-hide-forever-cb-" + Math.floor(10000 * Math.random());
    const hideCensorForeverLabel = document.createElement("label")
    hideCensorForeverLabel.innerText = "Don't show again";
    hideCensorForeverLabel.setAttribute("for", hideCensorForeverCheckbox.id);
    hideCensorForeverContainer.appendChild(hideCensorForeverCheckbox);
    hideCensorForeverContainer.appendChild(hideCensorForeverLabel);

    hideCensorButton.addEventListener("click", () => {
        if (hideCensorForeverCheckbox.checked)
            localStorage.setItem("hide_censor_forever", "true");
        setTimeout(() => uncensorCanvas({ canvasEl }), 0); // Prevent clicking restored a href instantly as well
    });

    censorContainerEl.appendChild(censorTitle);
    censorContainerEl.appendChild(censorText1);
    censorContainerEl.appendChild(censorText2);
    censorContainerEl.appendChild(hideCensorButton);
    censorContainerEl.appendChild(hideCensorForeverContainer);

    const reposition = () => {
        const canvasComputedStyle = getComputedStyle(canvasEl);
        // Update position (over canvas)
        censorWrapperContainerEl.style.left = `calc(${canvasEl.offsetLeft}px + ${canvasComputedStyle.marginLeft} + ${canvasComputedStyle.paddingLeft} + ${canvasComputedStyle.borderLeftWidth})`;
        censorWrapperContainerEl.style.top = `calc(${canvasEl.offsetTop}px + ${canvasComputedStyle.marginTop} + ${canvasComputedStyle.paddingTop} + ${canvasComputedStyle.borderTopWidth})`;
        censorWrapperContainerEl.style.width = canvasEl.clientWidth + "px";
        censorWrapperContainerEl.style.height = canvasEl.clientHeight + "px";
        censorWrapperContainerEl.style.fontSize = canvasEl.clientWidth / 25 + "px";
        censorWrapperContainerEl.style.backdropFilter = "blur(" + canvasEl.clientWidth / 20 + "px)";
    }
    reposition();
    censorState.windowResizeHandler = reposition;
    window.addEventListener("resize", reposition);
    censorState.resizeObserver = new ResizeObserver(reposition);
    censorState.resizeObserver.observe(canvasEl);

    console.log("Censored Canvas: " + description)
}
function uncensorCanvas() {
    if (!censorState.isCensored) return;

    censorState.isCensored = false;
    if (censorState.censorEl !== null) {
        censorState.censorEl.remove();
        censorState.censorEl = null;
    }
    if (censorState.originalParentHref !== null) {
        censorState.canvasEl.parentElement.setAttribute("href", censorState.originalParentHref);
        censorState.originalParentHref = null;
    }
    if (censorState.windowResizeHandler !== null) {
        window.removeEventListener("resize", censorState.windowResizeHandler);
        censorState.windowResizeHandler = null;
    }
    if (censorState.resizeObserver !== null) {
        censorState.resizeObserver.unobserve(censorState.canvasEl)
        censorState.resizeObserver = null;
    }

    censorState.canvasEl = null;
    console.log("Uncensored canvas!");
}
