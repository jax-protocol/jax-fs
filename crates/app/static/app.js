// Bucket Creation Module
const BucketCreation = {
  init(apiUrl) {
    const form = document.getElementById("createBucketForm");
    if (!form) return;

    form.addEventListener("submit", async (e) => {
      e.preventDefault();

      const nameInput = document.getElementById("bucketName");
      const status = document.getElementById("createStatus");
      const name = nameInput.value.trim();

      if (!name) {
        this.showStatus(status, "Please enter a bucket name", "error");
        return;
      }

      this.showStatus(status, "Creating bucket...", "info");

      try {
        const response = await fetch(`${apiUrl}/api/v0/bucket`, {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({ name: name }),
        });

        if (response.ok) {
          this.showStatus(
            status,
            "Bucket created successfully! Reloading...",
            "success",
          );
          setTimeout(() => window.location.reload(), 1000);
        } else {
          const error = await response.text();
          this.showStatus(status, "Failed to create bucket: " + error, "error");
        }
      } catch (error) {
        this.showStatus(
          status,
          "Failed to create bucket: " + error.message,
          "error",
        );
      }
    });
  },

  showStatus(element, message, type) {
    element.className =
      "p-4 " +
      (type === "error"
        ? "bg-red-100 text-red-800"
        : type === "success"
          ? "bg-green-100 text-green-800"
          : "bg-blue-100 text-blue-800");
    element.textContent = message;
    element.classList.remove("hidden");
  },
};

// File Upload Module
const FileUpload = {
  init(apiUrl, bucketId) {
    const form = document.getElementById("uploadForm");
    if (!form) return;

    form.addEventListener("submit", async (e) => {
      e.preventDefault();

      const fileInput = document.getElementById("fileInput");
      const status = document.getElementById("uploadStatus");

      if (!fileInput.files.length) {
        this.showStatus(status, "Please select a file", "error");
        return;
      }

      const file = fileInput.files[0];
      const path = window.JAX_CURRENT_PATH || "/";

      const formData = new FormData();
      formData.append("bucket_id", bucketId);
      formData.append("mount_path", path);
      formData.append("file", file);

      this.showStatus(status, "Uploading...", "info");

      try {
        const response = await fetch(`${apiUrl}/api/v0/bucket/add`, {
          method: "POST",
          body: formData,
        });

        if (response.ok) {
          this.showStatus(
            status,
            "File uploaded successfully! Reloading...",
            "success",
          );
          // setTimeout(() => window.location.reload(), 1000);
        } else {
          const error = await response.text();
          this.showStatus(status, "Upload failed: " + error, "error");
        }
      } catch (error) {
        this.showStatus(status, "Upload failed: " + error.message, "error");
      }
    });
  },

  showStatus(element, message, type) {
    element.className =
      "p-4 " +
      (type === "error"
        ? "bg-red-100 text-red-800"
        : type === "success"
          ? "bg-green-100 text-green-800"
          : "bg-blue-100 text-blue-800");
    element.textContent = message;
    element.classList.remove("hidden");
  },
};

// Bucket Share Module removed - now inline in share_modal.html

// File Rename Module
const FileRename = {
  init(apiUrl, bucketId) {
    const form = document.getElementById("renameForm");
    if (!form) return;

    form.addEventListener("submit", async (e) => {
      e.preventDefault();

      const oldPath = document.getElementById("renameOldPath").value;
      const newName = document.getElementById("renameNewName").value.trim();
      const status = document.getElementById("renameStatus");

      if (!newName) {
        this.showStatus(status, "Please enter a new name", "error");
        return;
      }

      // Build new path: parent_dir + new_name
      const oldPathParts = oldPath.split("/");
      oldPathParts.pop(); // Remove filename
      const parentPath = oldPathParts.join("/") || "/";
      const newPath =
        parentPath === "/" ? "/" + newName : parentPath + "/" + newName;

      this.showStatus(status, "Renaming...", "info");

      try {
        const response = await fetch(`${apiUrl}/api/v0/bucket/rename`, {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({
            bucket_id: bucketId,
            old_path: oldPath,
            new_path: newPath,
          }),
        });

        if (response.ok) {
          this.showStatus(
            status,
            "Renamed successfully! Reloading...",
            "success",
          );
          setTimeout(() => window.location.reload(), 1000);
        } else {
          const error = await response.text();
          this.showStatus(status, "Rename failed: " + error, "error");
        }
      } catch (error) {
        this.showStatus(status, "Rename failed: " + error.message, "error");
      }
    });
  },

  showStatus(element, message, type) {
    element.className =
      "p-4 " +
      (type === "error"
        ? "bg-red-100 text-red-800"
        : type === "success"
          ? "bg-green-100 text-green-800"
          : "bg-blue-100 text-blue-800");
    element.textContent = message;
    element.classList.remove("hidden");
  },
};

// New File Module
const NewFile = {
  init(apiUrl, bucketId, currentPath) {
    const form = document.getElementById("newFileForm");
    if (!form) return;

    form.addEventListener("submit", async (e) => {
      e.preventDefault();

      const fileNameInput = document.getElementById("newFileName");
      const contentInput = document.getElementById("newFileContent");
      const status = document.getElementById("newFileStatus");

      const fileName = fileNameInput.value.trim();
      const content = contentInput.value || "";

      if (!fileName) {
        this.showStatus(status, "Please enter a file name", "error");
        return;
      }

      // Validate file extension
      if (!fileName.endsWith(".txt") && !fileName.endsWith(".md")) {
        this.showStatus(
          status,
          "Only .txt and .md files are supported",
          "error",
        );
        return;
      }

      // Build the full path
      const path = currentPath.endsWith("/")
        ? currentPath + fileName
        : currentPath + "/" + fileName;

      this.showStatus(status, "Creating file...", "info");

      try {
        // Create a blob from the content
        const blob = new Blob([content], { type: "text/plain" });
        const file = new File([blob], fileName);

        // Upload using the add endpoint
        const formData = new FormData();
        formData.append("bucket_id", bucketId);
        formData.append("mount_path", path);
        formData.append("file", file);

        const response = await fetch(`${apiUrl}/api/v0/bucket/add`, {
          method: "POST",
          body: formData,
        });

        if (response.ok) {
          this.showStatus(
            status,
            "File created! Redirecting to editor...",
            "success",
          );
          // Redirect to editor
          setTimeout(() => {
            window.location.href = `/buckets/${bucketId}/edit?path=${encodeURIComponent(path)}`;
          }, 500);
        } else {
          const error = await response.text();
          this.showStatus(status, "Failed to create file: " + error, "error");
        }
      } catch (error) {
        this.showStatus(
          status,
          "Failed to create file: " + error.message,
          "error",
        );
      }
    });
  },

  showStatus(element, message, type) {
    element.className =
      "p-4 " +
      (type === "error"
        ? "bg-red-100 text-red-800"
        : type === "success"
          ? "bg-green-100 text-green-800"
          : "bg-blue-100 text-blue-800");
    element.textContent = message;
    element.classList.remove("hidden");
  },
};

// File Delete Module
const FileDelete = {
  init(apiUrl, bucketId) {
    // Delete is handled by global confirmDelete function
    window.confirmDelete = async () => {
      const path = document.getElementById("deleteItemPath").value;
      const status = document.getElementById("deleteStatus");

      this.showStatus(status, "Deleting...", "info");

      try {
        const response = await fetch(`${apiUrl}/api/v0/bucket/delete`, {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({
            bucket_id: bucketId,
            path: path,
          }),
        });

        if (response.ok) {
          this.showStatus(
            status,
            "Deleted successfully! Reloading...",
            "success",
          );
          setTimeout(() => window.location.reload(), 1000);
        } else {
          const error = await response.text();
          this.showStatus(status, "Delete failed: " + error, "error");
        }
      } catch (error) {
        this.showStatus(status, "Delete failed: " + error.message, "error");
      }
    };
  },

  showStatus(element, message, type) {
    element.className =
      "p-4 " +
      (type === "error"
        ? "bg-red-100 text-red-800"
        : type === "success"
          ? "bg-green-100 text-green-800"
          : "bg-blue-100 text-blue-800");
    element.textContent = message;
    element.classList.remove("hidden");
  },
};

// File Move Module
const FileMove = {
  directories: [],
  apiUrl: null,
  bucketId: null,

  init(apiUrl, bucketId) {
    this.apiUrl = apiUrl;
    this.bucketId = bucketId;

    const form = document.getElementById("moveForm");
    if (!form) return;

    form.addEventListener("submit", async (e) => {
      e.preventDefault();

      const sourcePath = document.getElementById("moveSourcePath").value;
      const destDir = document.getElementById("moveDestDir").value;
      const destName = document.getElementById("moveDestName").value.trim();
      const status = document.getElementById("moveStatus");

      if (!destName) {
        this.showStatus(status, "Please enter a name", "error");
        return;
      }

      // Build full destination path
      const destPath = destDir === "/" ? "/" + destName : destDir + "/" + destName;

      this.showStatus(status, "Moving...", "info");

      try {
        const response = await fetch(`${apiUrl}/api/v0/bucket/mv`, {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({
            bucket_id: bucketId,
            source_path: sourcePath,
            dest_path: destPath,
          }),
        });

        if (response.ok) {
          this.showStatus(
            status,
            "Moved successfully! Reloading...",
            "success",
          );
          setTimeout(() => window.location.reload(), 1000);
        } else {
          const error = await response.text();
          this.showStatus(status, "Move failed: " + error, "error");
        }
      } catch (error) {
        this.showStatus(status, "Move failed: " + error.message, "error");
      }
    });
  },

  async fetchDirectories() {
    if (!this.apiUrl || !this.bucketId) return;

    try {
      const response = await fetch(`${this.apiUrl}/api/v0/bucket/ls`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          bucket_id: this.bucketId,
          path: "/",
          deep: true,
        }),
      });

      if (response.ok) {
        const data = await response.json();
        // Filter to only directories and sort
        this.directories = data.items
          .filter((item) => item.is_dir)
          .map((item) => item.path)
          .sort();
      }
    } catch (error) {
      console.error("Failed to fetch directories:", error);
    }
  },

  populateDirectoryDropdown(currentDir) {
    const select = document.getElementById("moveDestDir");
    if (!select) return;

    // Clear existing options except root
    select.innerHTML = '<option value="/">/ (root)</option>';

    // Add directories
    this.directories.forEach((dir) => {
      const option = document.createElement("option");
      option.value = dir;
      option.textContent = dir;
      if (dir === currentDir) {
        option.selected = true;
      }
      select.appendChild(option);
    });
  },

  showStatus(element, message, type) {
    element.className =
      "p-4 " +
      (type === "error"
        ? "bg-red-100 text-red-800"
        : type === "success"
          ? "bg-green-100 text-green-800"
          : "bg-blue-100 text-blue-800");
    element.textContent = message;
    element.classList.remove("hidden");
  },
};

// File Editor Module removed - now using inline editor in file_viewer

// Global modal functions for bucket_explorer.html
function openRenameModal(path, name, isDir) {
  document.getElementById("renameOldPath").value = path;
  document.getElementById("renameNewName").value = name;
  document.getElementById("renameItemType").textContent = isDir
    ? "Directory"
    : "File";
  UIkit.modal("#rename-modal").show();
}

function openDeleteModal(path, name, isDir) {
  document.getElementById("deleteItemPath").value = path;
  document.getElementById("deleteItemName").textContent = name;
  document.getElementById("deleteItemType").textContent = isDir
    ? "Directory"
    : "File";
  UIkit.modal("#delete-modal").show();
}

async function openMoveModal(path, name, isDir) {
  document.getElementById("moveSourcePath").value = path;
  document.getElementById("moveSourceDisplay").value = path;
  document.getElementById("moveDestName").value = name;
  document.getElementById("moveItemType").textContent = isDir
    ? "Directory"
    : "File";

  // Get current directory from path
  const pathParts = path.split("/");
  pathParts.pop(); // Remove filename
  const currentDir = pathParts.join("/") || "/";

  // Fetch directories and populate dropdown
  await FileMove.fetchDirectories();
  FileMove.populateDirectoryDropdown(currentDir);

  UIkit.modal("#move-modal").show();
}

// Initialize modules when DOM is ready
document.addEventListener("DOMContentLoaded", function () {
  // Get API URL from data attribute on body or window
  const apiUrl = window.JAX_API_URL || "http://localhost:3000";
  const bucketId = window.JAX_BUCKET_ID;
  const currentPath = window.JAX_CURRENT_PATH || "/";

  BucketCreation.init(apiUrl);
  if (bucketId) {
    FileRename.init(apiUrl, bucketId);
    FileDelete.init(apiUrl, bucketId);
    FileMove.init(apiUrl, bucketId);
    NewFile.init(apiUrl, bucketId, currentPath);
  }
});
