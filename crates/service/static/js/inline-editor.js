/**
 * Inline Editor Module
 * Handles toggle between view/edit modes and saving functionality
 * Uses window.CodeMirror loaded from base.html
 */

let editorView = null;
let isEditing = false;
let hasUnsavedChanges = false;

/**
 * Initialize the inline editor
 */
export function initInlineEditor(bucketId, filePath, isMarkdown) {
    const toggleBtn = document.getElementById('toggleEditBtn');
    const saveBtn = document.getElementById('saveBtn');
    const viewMode = document.getElementById('viewMode');
    const editMode = document.getElementById('editMode');
    const toggleText = document.getElementById('toggleEditText');

    if (!toggleBtn || !saveBtn || !viewMode || !editMode) {
        console.warn('Editor elements not found, skipping initialization');
        return;
    }

    // Toggle edit mode
    toggleBtn.addEventListener('click', () => {
        if (isEditing) {
            // Switch to view mode
            switchToViewMode();
        } else {
            // Switch to edit mode
            switchToEditMode(isMarkdown);
        }
    });

    // Save changes
    saveBtn.addEventListener('click', async () => {
        if (!hasUnsavedChanges) return;

        const content = editorView.state.doc.toString();
        const success = await saveFile(bucketId, filePath, content);

        if (success) {
            // Update original content
            const originalContentElem = document.getElementById('originalContent');
            if (originalContentElem) {
                originalContentElem.value = content;
            }

            const markdownSourceElem = document.getElementById('markdownSource');
            if (isMarkdown && markdownSourceElem) {
                markdownSourceElem.value = content;
            }

            hasUnsavedChanges = false;
            saveBtn.disabled = true;

            // Update view mode content
            if (isMarkdown) {
                renderMarkdown(content);
            } else {
                const textContentElem = document.getElementById('textContent');
                if (textContentElem) {
                    textContentElem.textContent = content;
                }
            }

            // Switch back to view mode
            switchToViewMode();
        }
    });

    function switchToEditMode(isMarkdown) {
        isEditing = true;
        viewMode.classList.add('hidden');
        editMode.classList.remove('hidden');
        toggleText.textContent = 'Cancel';
        toggleBtn.classList.remove('button-primary');
        saveBtn.style.display = 'inline-flex';
        saveBtn.disabled = true; // Disable until changes are made

        // Initialize CodeMirror if not already done
        if (!editorView) {
            const editorContentElem = document.getElementById('editorContent');
            if (!editorContentElem) {
                console.error('Editor content element not found');
                return;
            }

            const editorParent = document.getElementById('editor');
            if (!editorParent) {
                console.error('Editor parent element not found');
                return;
            }

            // Wait for CodeMirror to be loaded from base.html
            if (!window.CodeMirror) {
                console.error('CodeMirror not loaded from base.html');
                return;
            }

            const { EditorView, basicSetup, markdown } = window.CodeMirror;

            const extensions = [basicSetup];

            if (isMarkdown && markdown) {
                extensions.push(markdown());
            }

            // Add change listener
            extensions.push(
                EditorView.updateListener.of((update) => {
                    if (update.docChanged) {
                        const currentContent = update.state.doc.toString();
                        const originalContentElem = document.getElementById('originalContent');
                        const originalContent = originalContentElem ? originalContentElem.value : '';
                        hasUnsavedChanges = currentContent !== originalContent;
                        saveBtn.disabled = !hasUnsavedChanges;
                    }
                })
            );

            editorView = new EditorView({
                doc: editorContentElem.value,
                extensions: extensions,
                parent: editorParent
            });
        }
    }

    function switchToViewMode() {
        isEditing = false;
        editMode.classList.add('hidden');
        viewMode.classList.remove('hidden');
        toggleText.textContent = 'Edit';
        toggleBtn.classList.add('button-primary');
        saveBtn.style.display = 'none';
        hasUnsavedChanges = false;

        // Reset editor content to original if changes weren't saved
        if (editorView) {
            const originalContentElem = document.getElementById('originalContent');
            const originalContent = originalContentElem ? originalContentElem.value : '';
            editorView.dispatch({
                changes: {
                    from: 0,
                    to: editorView.state.doc.length,
                    insert: originalContent
                }
            });
        }
    }

    async function saveFile(bucketId, filePath, content) {
        const apiUrl = window.JAX_API_URL || 'http://localhost:3000';

        const formData = new FormData();
        formData.append('bucket_id', bucketId);
        formData.append('mount_path', filePath);
        formData.append('file', new Blob([content], { type: 'text/plain' }), filePath.split('/').pop());

        try {
            const response = await fetch(`${apiUrl}/api/v0/bucket/update`, {
                method: 'POST',
                body: formData
            });

            if (!response.ok) {
                const errorText = await response.text();
                throw new Error(errorText || 'Failed to save file');
            }

            return true;
        } catch (error) {
            console.error('Error saving file:', error);
            alert('Failed to save file: ' + error.message);
            return false;
        }
    }
}

/**
 * Render markdown content
 */
export function renderMarkdown(content) {
    const markdownContent = document.getElementById('markdownContent');
    if (markdownContent && window.marked) {
        marked.setOptions({
            breaks: true,
            gfm: true,
            headerIds: true,
            mangle: false
        });
        markdownContent.innerHTML = marked.parse(content);
    }
}

/**
 * Initialize markdown rendering on page load
 */
export function initMarkdownRendering() {
    const markdownSource = document.getElementById('markdownSource');
    const markdownContent = document.getElementById('markdownContent');
    if (markdownSource && markdownContent && window.marked) {
        renderMarkdown(markdownSource.value);
    }
}
