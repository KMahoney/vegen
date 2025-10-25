(function () {
  function initTableOfContents() {
    const elements = getTocElements();
    if (!elements) {
      return;
    }

    const headings = collectVisibleHeadings(elements.contentRoot);
    if (!headings.length) {
      return;
    }

    const headingData = buildHeadingData(headings);
    if (!headingData.length) {
      return;
    }

    const contentSections = wrapContentSections(
      headingData,
      elements.contentRoot
    );
    const tree = buildTree(headingData, contentSections);
    if (!tree.allItems.length) {
      return;
    }

    const markerControls = renderNavigation(elements.tocNav, tree);
    elements.tocSidebar.classList.add("toc-sidebar--visible");

    markerControls.draw(true);
    window.requestAnimationFrame(function () {
      markerControls.draw(true);
    });
    window.setTimeout(function () {
      markerControls.draw(true);
    }, 250);
    window.addEventListener("resize", function () {
      window.requestAnimationFrame(function () {
        markerControls.draw(true);
      });
    });
    window.addEventListener("load", function () {
      markerControls.draw(true);
    });

    const initialHash = window.location.hash;
    let initialHashHandled = false;
    function attemptInitialHashScroll() {
      if (initialHashHandled || !initialHash) {
        return;
      }
      if (scrollHashIntoView(initialHash, "auto")) {
        initialHashHandled = true;
      }
    }
    if (initialHash) {
      window.requestAnimationFrame(attemptInitialHashScroll);
      window.setTimeout(attemptInitialHashScroll, 200);
      window.addEventListener(
        "load",
        function () {
          attemptInitialHashScroll();
        },
        { once: true }
      );
    }

    const markActiveStates = function () {
      updateActiveStates(tree, markerControls.sync);
    };

    const observer = createObserver(tree.itemById, markActiveStates);
    tree.allItems.forEach(function (item) {
      observeItem(observer, item);
    });
  }

  function getTocElements() {
    const tocSidebar = document.querySelector(".toc-sidebar");
    const tocNav = document.querySelector("[data-toc]");
    const contentRoot = document.querySelector(".content");
    if (!tocSidebar || !tocNav || !contentRoot) {
      return null;
    }
    return {
      tocSidebar: tocSidebar,
      tocNav: tocNav,
      contentRoot: contentRoot,
    };
  }

  function collectVisibleHeadings(contentRoot) {
    return Array.from(contentRoot.querySelectorAll("h1, h2")).filter(function (
      heading
    ) {
      return heading && heading.offsetParent !== null;
    });
  }

  function buildHeadingData(headings) {
    const headingData = [];
    const usedIds = new Set();
    headings.forEach(function (heading, index) {
      const text = getHeadingText(heading);
      if (!text) {
        return;
      }

      const depth = heading.tagName.toLowerCase() === "h2" ? 2 : 1;

      const id = ensureHeadingId(heading, text, index, usedIds);
      heading.dataset.targetId = id;

      headingData.push({
        id: id,
        depth: depth,
        text: text,
        heading: heading,
      });
    });
    return headingData;
  }

  function getHeadingText(heading) {
    const clone = heading.cloneNode(true);
    const anchorLink = clone.querySelector(".anchorjs-link");
    if (anchorLink) {
      anchorLink.remove();
    }
    return clone.textContent.replace(/\s+/g, " ").trim();
  }

  function ensureHeadingId(heading, text, index, usedIds) {
    if (heading.id) {
      usedIds.add(heading.id);
      return heading.id;
    }

    const baseSlug = slugify(text) || "section-" + (index + 1);
    let uniqueSlug = baseSlug;
    let counter = 2;
    while (usedIds.has(uniqueSlug) || document.getElementById(uniqueSlug)) {
      uniqueSlug = baseSlug + "-" + counter;
      counter += 1;
    }
    heading.id = uniqueSlug;
    usedIds.add(uniqueSlug);
    return uniqueSlug;
  }

  function slugify(text) {
    return text
      .toLowerCase()
      .trim()
      .replace(/[^\w\s-]/g, "")
      .replace(/\s+/g, "-");
  }

  function wrapContentSections(headingData, contentRoot) {
    const sectionMap = new Map();

    headingData.forEach(function (entry) {
      const heading = entry.heading;
      const id = entry.id;

      if (!heading || !heading.parentNode) {
        return;
      }

      const section = document.createElement("section");
      section.className =
        "content-section content-section--level" + entry.depth;
      section.dataset.depth = String(entry.depth);
      section.dataset.targetId = id;
      section.id = "section-" + id;
      contentRoot.insertBefore(section, heading);

      let current = heading;
      while (current) {
        const next = current.nextSibling;
        section.appendChild(current);
        if (!next) {
          break;
        }
        if (next.nodeType === 1) {
          const tag = next.tagName.toLowerCase();
          if (tag === "h1" || tag === "h2") {
            break;
          }
        }
        current = next;
      }

      sectionMap.set(id, section);
    });

    return sectionMap;
  }

  function buildTree(headingData, contentSections) {
    const rootItems = [];
    const allItems = [];
    const itemById = new Map();
    let lastTopLevel = null;

    headingData.forEach(function (entry) {
      const isTopLevel = entry.depth === 1;
      const parent = !isTopLevel && lastTopLevel ? lastTopLevel : null;

      const item = {
        id: entry.id,
        depth: entry.depth,
        text: entry.text,
        heading: entry.heading,
        parent: parent,
        children: [],
        navItem: null,
        link: null,
        contentSection: contentSections.get(entry.id) || null,
        active: false,
        headingVisible: false,
        sectionVisible: false,
        markerVisible: false,
      };

      if (parent) {
        parent.children.push(item);
      } else {
        rootItems.push(item);
      }

      allItems.push(item);
      itemById.set(item.id, item);
      if (isTopLevel) {
        lastTopLevel = item;
      }

      entry.heading.dataset.tocId = item.id;
    });

    return {
      rootItems: rootItems,
      allItems: allItems,
      itemById: itemById,
    };
  }

  function renderNavigation(tocNav, tree) {
    const fragment = document.createDocumentFragment();
    const listRoot = document.createElement("ul");
    listRoot.className = "toc-list";

    const marker = createMarker();
    let markerPathLength = 0;
    let lastMarkerStart = null;
    let lastMarkerEnd = null;

    function createLink(item) {
      const link = document.createElement("a");
      link.className = "toc-link";
      link.dataset.depth = String(item.depth);
      link.dataset.targetId = item.id;
      link.href = "#" + item.id;
      link.textContent = item.text;
      link.addEventListener(
        "click",
        function (event) {
          const sectionTarget = document.getElementById("section-" + item.id);
          const headingTarget = document.getElementById(item.id);
          const target = sectionTarget || headingTarget;
          if (!target) {
            return;
          }
          target.scrollIntoView({ behavior: "smooth", block: "start" });
          window.setTimeout(function () {
            window.location.hash = item.id;
          }, 350);
        },
        { passive: true }
      );
      return link;
    }

    function renderItems(items, targetList) {
      items.forEach(function (item) {
        const li = document.createElement("li");
        li.className = "toc-item toc-item--level" + item.depth;
        li.dataset.depth = String(item.depth);
        li.dataset.targetId = item.id;

        const link = createLink(item);
        li.appendChild(link);

        item.navItem = li;
        item.link = link;

        if (item.children.length) {
          const childList = document.createElement("ul");
          childList.className =
            "toc-sublist toc-sublist--level" + (item.depth + 1);
          renderItems(item.children, childList);
          li.appendChild(childList);
        }

        targetList.appendChild(li);
      });
    }

    function resetMarker() {
      marker.path.removeAttribute("d");
      markerPathLength = 0;
      lastMarkerStart = null;
      lastMarkerEnd = null;
      marker.path.style.opacity = "0";
    }

    function syncMarker(force) {
      if (!markerPathLength) {
        marker.path.style.opacity = "0";
        return;
      }

      let pathStart = markerPathLength;
      let pathEnd = 0;
      let visibleItems = 0;

      tree.allItems.forEach(function (item) {
        if (
          typeof item.pathStart !== "number" ||
          typeof item.pathEnd !== "number"
        ) {
          return;
        }
        if (item.markerVisible) {
          pathStart = Math.min(pathStart, item.pathStart);
          pathEnd = Math.max(pathEnd, item.pathEnd);
          visibleItems += 1;
        }
      });

      if (!visibleItems) {
        marker.path.style.opacity = "0";
        lastMarkerStart = null;
        lastMarkerEnd = null;
        return;
      }

      const dashArray =
        "1, " +
        pathStart +
        ", " +
        (pathEnd - pathStart) +
        ", " +
        markerPathLength;

      if (
        force ||
        lastMarkerStart === null ||
        lastMarkerEnd === null ||
        pathStart !== lastMarkerStart ||
        pathEnd !== lastMarkerEnd
      ) {
        marker.path.setAttribute("stroke-dasharray", dashArray);
        marker.path.setAttribute("stroke-dashoffset", "1");
        lastMarkerStart = pathStart;
        lastMarkerEnd = pathEnd;
      }
      marker.path.style.opacity = "1";
    }

    function drawMarkerPath(force) {
      const items = tree.allItems.filter(function (item) {
        return item.navItem && item.link;
      });
      if (!items.length) {
        resetMarker();
        return;
      }

      const navRect = tocNav.getBoundingClientRect();
      const width = navRect.width;
      const height = navRect.height;
      if (!width || !height) {
        marker.path.style.opacity = "0";
        return;
      }

      marker.svg.setAttribute("width", String(width));
      marker.svg.setAttribute("height", String(height));
      marker.svg.setAttribute("viewBox", "0 0 " + width + " " + height);

      let pathData = "";
      let previousX = 0;
      let lastLength = 0;

      items.forEach(function (item, index) {
        const linkRect = item.link.getBoundingClientRect();
        const x = linkRect.left - navRect.left;
        const y = linkRect.top - navRect.top;
        const bottom = y + linkRect.height;
        const roundedX = Math.round(x * 10) / 10;
        const roundedY = Math.round(y * 10) / 10;
        const roundedBottom = Math.round(bottom * 10) / 10;

        if (index === 0) {
          pathData +=
            "M " +
            roundedX +
            " " +
            roundedY +
            " L " +
            roundedX +
            " " +
            roundedBottom;
        } else {
          const roundedPrevX = Math.round(previousX * 10) / 10;
          pathData += " L " + roundedPrevX + " " + roundedY;
          if (Math.abs(previousX - roundedX) > 0.5) {
            pathData += " L " + roundedX + " " + roundedY;
          }
          pathData += " L " + roundedX + " " + roundedBottom;
        }

        marker.path.setAttribute("d", pathData);
        const pathTotal = marker.path.getTotalLength();
        item.pathStart = lastLength;
        item.pathEnd = pathTotal;
        lastLength = pathTotal;
        previousX = roundedX;
      });

      markerPathLength = lastLength;
      syncMarker(force);
    }

    renderItems(tree.rootItems, listRoot);
    fragment.appendChild(listRoot);
    fragment.appendChild(marker.svg);
    tocNav.textContent = "";
    tocNav.appendChild(fragment);

    return {
      draw: function (force) {
        drawMarkerPath(Boolean(force));
      },
      sync: function (force) {
        syncMarker(Boolean(force));
      },
    };
  }

  function createMarker() {
    const svgNS = "http://www.w3.org/2000/svg";
    const svg = document.createElementNS(svgNS, "svg");
    svg.classList.add("toc-marker");
    svg.setAttribute("aria-hidden", "true");
    svg.setAttribute("focusable", "false");
    const path = document.createElementNS(svgNS, "path");
    path.setAttribute("stroke-dashoffset", "1");
    svg.appendChild(path);
    return {
      svg: svg,
      path: path,
    };
  }

  function updateActiveStates(tree, syncMarker) {
    function computeState(item) {
      let childActive = false;
      item.children.forEach(function (child) {
        if (computeState(child)) {
          childActive = true;
        }
      });
      const selfVisible = item.headingVisible || item.sectionVisible;
      const active = selfVisible || (item.depth > 1 && childActive);
      item.active = active;
      item.markerVisible = selfVisible;
      return active;
    }

    tree.rootItems.forEach(computeState);
    tree.allItems.forEach(function (item) {
      if (item.link) {
        item.link.classList.toggle("is-active", item.active);
      }
      if (item.navItem) {
        item.navItem.classList.toggle("is-active", item.active);
        item.navItem.classList.toggle("visible", item.active);
      }
    });

    syncMarker();
  }

  function createObserver(itemById, onChange) {
    return new IntersectionObserver(
      function (entries) {
        let changed = false;
        entries.forEach(function (entry) {
          const target = entry.target;
          if (!target) {
            return;
          }
          const dataset = target.dataset || {};
          const targetId = dataset.targetId || target.id;
          if (!targetId) {
            return;
          }
          const item = itemById.get(targetId);
          if (!item) {
            return;
          }
          const observeType = dataset.tocObserverType || "section";
          const visible = entry.isIntersecting && entry.intersectionRatio > 0;
          if (observeType === "heading") {
            if (item.headingVisible !== visible) {
              item.headingVisible = visible;
              changed = true;
            }
          } else {
            if (item.sectionVisible !== visible) {
              item.sectionVisible = visible;
              changed = true;
            }
          }
        });
        if (changed) {
          onChange();
        }
      },
      {
        threshold: [0, 0.15, 0.5],
      }
    );
  }

  function observeItem(observer, item) {
    const headingTarget = item.heading;
    if (headingTarget) {
      if (headingTarget.dataset) {
        headingTarget.dataset.targetId = item.id;
        headingTarget.dataset.tocObserverType = "heading";
      }
      observer.observe(headingTarget);
    }
    const sectionTarget = item.contentSection;
    if (sectionTarget) {
      if (sectionTarget.dataset) {
        sectionTarget.dataset.targetId = item.id;
        sectionTarget.dataset.tocObserverType = "section";
      }
      observer.observe(sectionTarget);
    }
  }

  function resolveHashTarget(hash) {
    if (!hash || hash.length < 2) {
      return null;
    }
    const rawId = hash.slice(1);
    if (!rawId) {
      return null;
    }
    let decodedId = rawId;
    try {
      decodedId = decodeURIComponent(rawId);
    } catch (error) {
      decodedId = rawId;
    }
    const sectionTarget = document.getElementById("section-" + decodedId);
    if (sectionTarget) {
      return sectionTarget;
    }
    return document.getElementById(decodedId);
  }

  function scrollHashIntoView(hash, behavior) {
    const target = resolveHashTarget(hash);
    if (!target) {
      return false;
    }
    target.scrollIntoView({
      behavior: behavior || "auto",
      block: "start",
    });
    return true;
  }

  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", initTableOfContents);
  } else {
    initTableOfContents();
  }
})();
