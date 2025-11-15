// Tree Viewer for Bucket History DAG
(function () {
  "use strict";

  const tableViewBtn = document.getElementById("tableViewBtn");
  const treeViewBtn = document.getElementById("treeViewBtn");
  const tableView = document.getElementById("tableView");
  const treeView = document.getElementById("treeView");
  const treeContainer = document.getElementById("treeContainer");
  const paginationControls = document.getElementById("paginationControls");

  let treeData = null;

  // Get initial view from URL params
  const urlParams = new URLSearchParams(window.location.search);
  const initialView = urlParams.get("view") || "table";

  // View switching
  function switchToTableView() {
    tableView.classList.remove("hidden");
    treeView.classList.add("hidden");
    paginationControls.classList.remove("hidden");
    tableViewBtn.classList.add("button-primary");
    treeViewBtn.classList.remove("button-primary");

    // Update URL without reload
    const url = new URL(window.location);
    url.searchParams.set("view", "table");
    window.history.pushState({}, "", url);
  }

  function switchToTreeView() {
    tableView.classList.add("hidden");
    treeView.classList.remove("hidden");
    paginationControls.classList.add("hidden");
    tableViewBtn.classList.remove("button-primary");
    treeViewBtn.classList.add("button-primary");

    // Update URL without reload
    const url = new URL(window.location);
    url.searchParams.set("view", "tree");
    window.history.pushState({}, "", url);

    if (!treeData) {
      loadTreeData();
    }
  }

  tableViewBtn.addEventListener("click", switchToTableView);
  treeViewBtn.addEventListener("click", switchToTreeView);

  // Initialize view based on URL param
  if (initialView === "tree") {
    // Set initial state without animation
    tableView.classList.add("hidden");
    treeView.classList.remove("hidden");
    paginationControls.classList.add("hidden");
    tableViewBtn.classList.remove("button-primary");
    treeViewBtn.classList.add("button-primary");

    loadTreeData();
  } else {
    // Ensure pagination is visible for table view
    paginationControls.classList.remove("hidden");
  }

  // Load tree data from API
  async function loadTreeData() {
    try {
      const response = await fetch(`${window.JAX_API_URL}/api/v0/bucket/tree`, {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
        },
        body: JSON.stringify({
          bucket_id: window.JAX_BUCKET_ID,
        }),
      });
      if (!response.ok) {
        throw new Error("Failed to load tree data");
      }
      treeData = await response.json();
      renderTree(treeData);
    } catch (error) {
      treeContainer.innerHTML = `
        <div class="text-center text-red-600 p-8">
          <i class="fas fa-exclamation-triangle text-2xl mb-4"></i>
          <p>Error loading tree: ${error.message}</p>
        </div>
      `;
    }
  }

  // Render the tree visualization as a graph
  function renderTree(data) {
    if (!data.nodes || data.nodes.length === 0) {
      treeContainer.innerHTML =
        '<div class="text-center text-muted-foreground p-8">No tree data available</div>';
      return;
    }

    // Build parent-child relationships
    const nodeMap = new Map();
    data.nodes.forEach((node) => {
      nodeMap.set(node.link, node);
    });

    // Group nodes by height and sort descending (newest first)
    const nodesByHeight = new Map();
    data.nodes.forEach((node) => {
      if (!nodesByHeight.has(node.height)) {
        nodesByHeight.set(node.height, []);
      }
      nodesByHeight.get(node.height).push(node);
    });

    const heights = Array.from(nodesByHeight.keys()).sort((a, b) => b - a);

    // Assign lanes to each node for horizontal positioning
    const nodeLanes = assignLanes(data.nodes, nodesByHeight, heights);
    const maxLane = Math.max(...Array.from(nodeLanes.values()));

    // Create the graph container
    const graph = document.createElement("div");
    graph.className = "tree-graph";

    // Render each height level
    heights.forEach((height, heightIndex) => {
      const nodesAtHeight = nodesByHeight.get(height);

      nodesAtHeight.forEach((node) => {
        const row = document.createElement("div");
        row.className = "tree-row";

        // Height label
        const heightLabel = document.createElement("div");
        heightLabel.className = "tree-height";
        heightLabel.textContent = `#${height}`;
        row.appendChild(heightLabel);

        // Graph columns (lanes)
        for (let lane = 0; lane <= maxLane; lane++) {
          const col = document.createElement("div");
          col.className = "tree-graph-col";

          const nodeLane = nodeLanes.get(node.link);

          // Draw commit dot in this lane
          if (lane === nodeLane) {
            const commit = document.createElement("div");
            commit.className = `tree-commit${node.is_canonical ? " canonical" : ""}`;
            col.appendChild(commit);
          }

          // Draw vertical line if there's a child below
          const childrenBelow = heights
            .slice(heightIndex + 1)
            .flatMap((h) => nodesByHeight.get(h))
            .filter((child) => {
              const childLane = nodeLanes.get(child.link);
              return childLane === lane && child.parent;
            });

          if (childrenBelow.length > 0) {
            const line = document.createElement("div");
            line.className = `tree-line tree-line-vertical${childrenBelow.some((c) => c.is_canonical) ? " canonical" : ""}`;
            line.style.top = "50%";
            line.style.height = "60px";
            col.appendChild(line);
          }

          // Draw line to parent
          if (lane === nodeLane && node.parent) {
            const parentNode = nodeMap.get(node.parent);
            if (parentNode) {
              const parentLane = nodeLanes.get(parentNode.link);

              // Vertical line going up
              const lineUp = document.createElement("div");
              lineUp.className = `tree-line tree-line-vertical${node.is_canonical ? " canonical" : ""}`;
              lineUp.style.bottom = "50%";
              lineUp.style.height = "30px";
              col.appendChild(lineUp);
            }
          }

          row.appendChild(col);
        }

        // Commit info
        const info = document.createElement("div");
        info.className = `tree-info${node.is_canonical ? " canonical" : ""}`;
        info.onclick = () => {
          window.location.href = `/buckets/${window.JAX_BUCKET_ID}?at=${node.link}`;
        };

        const hash = document.createElement("code");
        hash.className = "tree-hash";
        hash.textContent = node.link.slice(0, 8);
        info.appendChild(hash);

        const name = document.createElement("span");
        name.className = "tree-name";
        name.textContent = node.name;
        info.appendChild(name);

        if (node.link === data.canonical_head) {
          const badge = document.createElement("span");
          badge.className = "tree-badge";
          badge.textContent = "HEAD";
          info.appendChild(badge);
        }

        const date = document.createElement("span");
        date.className = "tree-date";
        date.textContent = new Date(node.created_at).toLocaleString();
        info.appendChild(date);

        row.appendChild(info);
        graph.appendChild(row);
      });
    });

    treeContainer.innerHTML = "";
    treeContainer.appendChild(graph);
  }

  // Assign each node to a horizontal lane (column)
  function assignLanes(nodes, nodesByHeight, heights) {
    const nodeLanes = new Map();
    let nextLane = 0;

    // Process from genesis (oldest) to head (newest)
    for (let i = heights.length - 1; i >= 0; i--) {
      const height = heights[i];
      const nodesAtHeight = nodesByHeight.get(height);

      nodesAtHeight.forEach((node) => {
        if (nodeLanes.has(node.link)) {
          return;
        }

        if (!node.parent) {
          // Genesis node
          nodeLanes.set(node.link, nextLane++);
        } else {
          // Find parent's lane
          const parentLane = nodeLanes.get(node.parent);
          if (parentLane !== undefined) {
            // Check if parent lane is free at this height
            const siblingsInParentLane = nodesAtHeight.filter(
              (n) => nodeLanes.get(n.link) === parentLane,
            );
            if (siblingsInParentLane.length === 0) {
              // Use parent's lane
              nodeLanes.set(node.link, parentLane);
            } else {
              // Fork - assign new lane
              nodeLanes.set(node.link, nextLane++);
            }
          } else {
            // Parent not assigned yet
            nodeLanes.set(node.link, nextLane++);
          }
        }
      });
    }

    return nodeLanes;
  }
})();
