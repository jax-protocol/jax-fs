// Note Editor Module
// Handles the dedicated markdown note editor with preview

// Use CodeMirror loaded globally from base.html
const { EditorView, basicSetup, markdown, oneDark } = window.CodeMirror || {};

let editorView = null;
let editorConfig = {};
let currentMode = 'edit'; // 'edit', 'preview', 'split'
let hasUnsavedChanges = false;

export function initNoteEditor(config) {
    editorConfig = config;

    // Initialize CodeMirror
    initCodeMirror();

    // Setup mode toggle buttons
    setupModeToggle();

    // Setup filename editing
    setupFilenameInput();

    // Setup save/cancel buttons
    setupActionButtons();

    // Setup unsaved changes warning
    setupUnsavedChangesWarning();

    // Load initial content
    loadInitialContent();
}

function initCodeMirror() {
    const editorElement = document.getElementById('editor');
    if (!editorElement) {
        console.error('Editor element not found');
        return;
    }

    // Check if CodeMirror is loaded
    if (!window.CodeMirror || !EditorView) {
        console.error('CodeMirror not loaded. Make sure base.html includes CodeMirror script.');
        return;
    }

    // Detect dark mode
    const isDark = document.documentElement.classList.contains('dark');

    const extensions = [
        basicSetup,
        markdown(),
        EditorView.updateListener.of((update) => {
            if (update.docChanged) {
                hasUnsavedChanges = true;
                if (currentMode === 'preview' || currentMode === 'split') {
                    updatePreview();
                }
            }
        })
    ];

    if (isDark) {
        extensions.push(oneDark);
    }

    editorView = new EditorView({
        doc: '',
        extensions,
        parent: editorElement
    });
}

function loadInitialContent() {
    // Read content from hidden textarea
    const contentElement = document.getElementById('initialContent');
    const initialContent = contentElement ? contentElement.value : '';

    if (editorView && initialContent) {
        editorView.dispatch({
            changes: { from: 0, to: editorView.state.doc.length, insert: initialContent }
        });
    }
}

function setupModeToggle() {
    const editBtn = document.getElementById('editModeBtn');
    const previewBtn = document.getElementById('previewModeBtn');
    const splitBtn = document.getElementById('splitModeBtn');

    const editorPane = document.getElementById('editor');
    const previewPane = document.getElementById('preview');

    editBtn.addEventListener('click', () => {
        currentMode = 'edit';
        editBtn.classList.add('active');
        previewBtn.classList.remove('active');
        splitBtn.classList.remove('active');

        editorPane.style.display = 'block';
        previewPane.style.display = 'none';
        editorPane.style.flex = '1';
    });

    previewBtn.addEventListener('click', () => {
        currentMode = 'preview';
        editBtn.classList.remove('active');
        previewBtn.classList.add('active');
        splitBtn.classList.remove('active');

        editorPane.style.display = 'none';
        previewPane.style.display = 'block';
        previewPane.style.flex = '1';

        updatePreview();
    });

    splitBtn.addEventListener('click', () => {
        currentMode = 'split';
        editBtn.classList.remove('active');
        previewBtn.classList.remove('active');
        splitBtn.classList.add('active');

        editorPane.style.display = 'block';
        previewPane.style.display = 'block';
        editorPane.style.flex = '1';
        previewPane.style.flex = '1';

        updatePreview();
    });
}

function updatePreview() {
    if (!editorView) return;

    const content = editorView.state.doc.toString();
    const previewPane = document.getElementById('preview');

    if (window.marked) {
        previewPane.innerHTML = marked.parse(content);
    } else {
        previewPane.textContent = content;
    }
}

function setupFilenameInput() {
    const filenameInput = document.getElementById('fileNameInput');

    filenameInput.addEventListener('input', () => {
        hasUnsavedChanges = true;
    });

    // Validate .md extension
    filenameInput.addEventListener('blur', () => {
        let filename = filenameInput.value.trim();

        if (!filename) {
            filenameInput.value = editorConfig.originalFilename;
            return;
        }

        // Ensure .md extension
        if (!filename.endsWith('.md')) {
            filename += '.md';
            filenameInput.value = filename;
        }
    });
}

function setupActionButtons() {
    const saveBtn = document.getElementById('saveBtn');
    const cancelBtn = document.getElementById('cancelBtn');

    saveBtn.addEventListener('click', async () => {
        await saveNote();
    });

    cancelBtn.addEventListener('click', () => {
        if (hasUnsavedChanges) {
            if (!confirm('You have unsaved changes. Are you sure you want to cancel?')) {
                return;
            }
        }
        goBack();
    });
}

async function saveNote() {
    const saveBtn = document.getElementById('saveBtn');
    const filenameInput = document.getElementById('fileNameInput');

    const newFilename = filenameInput.value.trim();
    const content = editorView ? editorView.state.doc.toString() : '';

    // Validate filename
    if (!newFilename) {
        alert('Please enter a filename');
        return;
    }

    if (!newFilename.endsWith('.md')) {
        alert('Filename must end with .md');
        return;
    }

    // Disable save button
    saveBtn.disabled = true;
    saveBtn.innerHTML = '<i class="fas fa-spinner fa-spin"></i> Saving...';

    try {
        // Determine the final path
        let finalPath;
        if (editorConfig.currentPath === '/') {
            finalPath = `/${newFilename}`;
        } else {
            finalPath = `${editorConfig.currentPath}/${newFilename}`;
        }

        // Check if we need to rename (filename changed)
        const filenameChanged = newFilename !== editorConfig.originalFilename;

        if (editorConfig.isNewFile) {
            // Create new file
            await createFile(finalPath, content);
        } else if (filenameChanged) {
            // Rename + update content
            await renameAndUpdateFile(editorConfig.filePath, finalPath, content);
        } else {
            // Just update content
            await updateFile(editorConfig.filePath, content);
        }

        hasUnsavedChanges = false;

        // Redirect back - use new path if file was renamed/created
        if (editorConfig.isNewFile || filenameChanged) {
            goBack(finalPath);
        } else {
            goBack();
        }
    } catch (error) {
        console.error('Save error:', error);
        alert(`Failed to save: ${error.message}`);

        // Re-enable save button
        saveBtn.disabled = false;
        saveBtn.innerHTML = '<i class="fas fa-save"></i> Save';
    }
}

async function createFile(path, content) {
    const formData = new FormData();
    formData.append('bucket_id', editorConfig.bucketId);
    formData.append('mount_path', path);

    // Create a Blob from the content
    const blob = new Blob([content], { type: 'text/markdown' });
    formData.append('file', blob);

    const response = await fetch(`${editorConfig.apiUrl}/api/v0/bucket/add`, {
        method: 'POST',
        body: formData
    });

    if (!response.ok) {
        const error = await response.text();
        throw new Error(error || 'Failed to create file');
    }
}

async function updateFile(path, content) {
    const formData = new FormData();
    formData.append('bucket_id', editorConfig.bucketId);
    formData.append('mount_path', path);

    const blob = new Blob([content], { type: 'text/markdown' });
    formData.append('file', blob);

    const response = await fetch(`${editorConfig.apiUrl}/api/v0/bucket/update`, {
        method: 'POST',
        body: formData
    });

    if (!response.ok) {
        const error = await response.text();
        throw new Error(error || 'Failed to update file');
    }
}

async function renameAndUpdateFile(oldPath, newPath, content) {
    // First rename
    const renamePayload = {
        bucket_id: editorConfig.bucketId,
        old_path: oldPath,
        new_path: newPath
    };

    const renameResponse = await fetch(`${editorConfig.apiUrl}/api/v0/bucket/rename`, {
        method: 'POST',
        headers: {
            'Content-Type': 'application/json'
        },
        body: JSON.stringify(renamePayload)
    });

    if (!renameResponse.ok) {
        const error = await renameResponse.text();
        throw new Error(error || 'Failed to rename file');
    }

    // Then update content at new path
    await updateFile(newPath, content);
}

function setupUnsavedChangesWarning() {
    window.addEventListener('beforeunload', (e) => {
        if (hasUnsavedChanges) {
            e.preventDefault();
            e.returnValue = '';
        }
    });
}

function goBack(newFilePath) {
    // If a new file path is provided (after rename/create), update the back URL accordingly
    if (newFilePath && editorConfig.backUrl && editorConfig.backUrl.includes('/view?path=')) {
        // We came from viewer and the file was renamed, so go to the new viewer path
        window.location.href = `/buckets/${editorConfig.bucketId}/view?path=${newFilePath}`;
    } else if (newFilePath) {
        // New file created or renamed, but we came from directory listing
        window.location.href = `/buckets/${editorConfig.bucketId}?path=${editorConfig.currentPath}`;
    } else {
        // No rename, use the original back URL
        const backUrl = editorConfig.backUrl || `/buckets/${editorConfig.bucketId}?path=${editorConfig.currentPath}`;
        window.location.href = backUrl;
    }
}
