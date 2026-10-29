// 把 mdbook 渲染出的 ```mermaid 代码块转成 Mermaid 图表
// mermaid 库按需从 CDN 加载，页面里没有 mermaid 代码块时不会请求
(function () {
    "use strict";

    // 固定版本，避免上游升级导致图表样式/语法变化
    var MERMAID_CDN = "https://cdn.jsdelivr.net/npm/mermaid@11.17.2/dist/mermaid.min.js";

    var loaded = false;
    var loading = null;
    var themeTimer = null;

    // 深色主题下用 dark 配色，其余用 default
    function isDarkTheme() {
        var cls = document.documentElement.className || "";
        return /navy|coal|ayu/.test(cls);
    }

    // mdbook 把代码块渲染成 <pre><code class="language-mermaid">，转成 <div class="mermaid">
    function convertCodeBlocks() {
        document.querySelectorAll("pre code.language-mermaid").forEach(function (code) {
            var block = document.createElement("div");
            block.className = "mermaid";
            block.textContent = code.textContent;
            code.parentElement.replaceWith(block);
        });
    }

    function loadMermaid() {
        if (loaded) {
            return Promise.resolve();
        }
        if (loading) {
            return loading;
        }
        loading = new Promise(function (resolve, reject) {
            var script = document.createElement("script");
            script.src = MERMAID_CDN;
            script.onload = function () {
                loaded = true;
                resolve();
            };
            script.onerror = function () {
                loading = null;
                reject(new Error("mermaid 加载失败: " + MERMAID_CDN));
            };
            document.head.appendChild(script);
        });
        return loading;
    }

    function renderAll() {
        if (!loaded || typeof mermaid === "undefined") {
            return;
        }

        var nodes = [];
        document.querySelectorAll("div.mermaid").forEach(function (node) {
            // 首次渲染时记录原始定义，主题切换后需要重新绘制
            if (!node.dataset.src) {
                node.dataset.src = node.textContent;
            }
            node.removeAttribute("data-processed");
            node.textContent = node.dataset.src;
            nodes.push(node);
        });
        if (!nodes.length) {
            return;
        }

        mermaid.initialize({
            startOnLoad: false,
            theme: isDarkTheme() ? "dark" : "default"
        });
        mermaid.run({ nodes: nodes, suppressErrors: true });
    }

    function start() {
        if (!document.querySelector("pre code.language-mermaid")) {
            return;
        }
        convertCodeBlocks();

        loadMermaid().then(renderAll).catch(function (err) {
            console.error(err);
        });

        // 切换主题时 documentElement 的 class 会变化，重新绘制以匹配配色
        new MutationObserver(function () {
            clearTimeout(themeTimer);
            themeTimer = setTimeout(renderAll, 100);
        }).observe(document.documentElement, { attributes: true, attributeFilter: ["class"] });
    }

    if (document.readyState === "loading") {
        document.addEventListener("DOMContentLoaded", start);
    } else {
        start();
    }
})();
