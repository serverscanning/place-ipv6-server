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
    const censorContainerEl = document.createElement("div");

    canvasEl.parentElement.insertBefore(censorWrapperContainerEl, canvasEl);
    censorState.censorEl = censorWrapperContainerEl;
    censorWrapperContainerEl.style.position = "absolute";
    censorWrapperContainerEl.style.backdropFilter = "blur(35px)";

    censorContainerEl.style.padding = "5px";
    censorContainerEl.style.width = "100%";
    censorContainerEl.style.height = "100%";
    censorContainerEl.style.boxSizing = "border-box";
    censorWrapperContainerEl.appendChild(censorContainerEl);

    // Align children
    censorContainerEl.style.display = "flex";
    censorContainerEl.style.flexDirection = "column";
    censorContainerEl.style.justifyContent = "center";
    censorContainerEl.style.alignItems = "center";
    // Default text styles
    censorContainerEl.style.color = "white";
    censorContainerEl.style.textShadow = "1px 1px 0px black";

    const censorTitle = document.createElement("h1");
    censorTitle.innerText = title;
    censorTitle.style.color = "red";
    censorTitle.style.textShadow = "1px 1px 0px black";

    const censorText1 = document.createElement("p");
    if (descriptionIsHtml)
        censorText1.innerHTML = description;
    else
        censorText1.innerText = description;
    censorText1.style.color = "white";
    censorText1.style.textShadow = "1px 1px 0px black";

    const censorText2 = document.createElement("p");
    censorText2.innerText = "If you're a minor, DO NOT click the button blow. The content could be nasty! This will disappear once the content is safe again.";
    censorText2.style.color = "white";
    censorText2.style.textShadow = "1px 1px 0px black";

    const hideCensorButton = document.createElement("a");
    hideCensorButton.style.boxShadow = "1px 1px 0px black";
    hideCensorButton.style.border = "1px solid white";
    hideCensorButton.style.borderRadius = "5px";
    hideCensorButton.style.padding = "0 3px";
    hideCensorButton.style.color = "white";
    hideCensorButton.style.backgroundColor = "rgba(255, 255, 255, 0.25)";
    hideCensorButton.style.width = "fit-content";
    hideCensorButton.style.margin = "0 auto";
    hideCensorButton.style.marginTop = "24px";
    hideCensorButton.style.cursor = "pointer";
    hideCensorButton.innerText = "Show anyway";

    const hideCensorForeverContainer = document.createElement("div");
    hideCensorForeverContainer.style.margin = "3px 0";
    const hideCensorForeverCheckbox = document.createElement("input")
    hideCensorForeverCheckbox.type = "checkbox";
    hideCensorForeverCheckbox.id = "hide-censor-forever-" + Math.floor(10000 * Math.random());
    const hideCensorForeverLabel = document.createElement("label")
    hideCensorForeverLabel.innerText = "Don't show again";
    hideCensorForeverLabel.setAttribute("for", hideCensorForeverCheckbox.id);
    hideCensorForeverLabel.style.fontSize = "10pt";
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
        censorWrapperContainerEl.style.width = canvasEl.width + "px";
        censorWrapperContainerEl.style.height = canvasEl.height + "px";
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
        resizeObserver.unobserve(censorState.canvasEl)
        censorState.resizeObserver = null;
    }

    censorState.canvasEl = null;
    console.log("Uncensored canvas!");
}
