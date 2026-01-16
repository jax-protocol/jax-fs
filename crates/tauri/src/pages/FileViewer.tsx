import { A, useParams } from "@solidjs/router";
import {
  Component,
  createResource,
  createMemo,
  Show,
  Match,
  Switch,
} from "solid-js";
import { getBucket, getFile } from "../hooks/useTauri";

// Helper to decode bytes to string
function bytesToString(bytes: number[]): string {
  return new TextDecoder().decode(new Uint8Array(bytes));
}

// Helper to create data URL from bytes
function bytesToDataUrl(bytes: number[], mimeType: string): string {
  const base64 = btoa(String.fromCharCode(...bytes));
  return `data:${mimeType};base64,${base64}`;
}

// Determine file type from extension
function getFileType(
  path: string
): "text" | "markdown" | "image" | "code" | "unknown" {
  const ext = path.split(".").pop()?.toLowerCase() || "";

  const imageExts = ["png", "jpg", "jpeg", "gif", "webp", "svg", "ico", "bmp"];
  const markdownExts = ["md", "markdown"];
  const codeExts = [
    "js",
    "ts",
    "jsx",
    "tsx",
    "rs",
    "py",
    "go",
    "java",
    "c",
    "cpp",
    "h",
    "hpp",
    "css",
    "scss",
    "json",
    "yaml",
    "yml",
    "toml",
    "xml",
    "html",
    "sh",
    "bash",
    "zsh",
    "sql",
  ];
  const textExts = ["txt", "log", "csv", "env"];

  if (imageExts.includes(ext)) return "image";
  if (markdownExts.includes(ext)) return "markdown";
  if (codeExts.includes(ext)) return "code";
  if (textExts.includes(ext)) return "text";

  return "unknown";
}

// Get MIME type for images
function getImageMimeType(path: string): string {
  const ext = path.split(".").pop()?.toLowerCase() || "";
  const mimeTypes: Record<string, string> = {
    png: "image/png",
    jpg: "image/jpeg",
    jpeg: "image/jpeg",
    gif: "image/gif",
    webp: "image/webp",
    svg: "image/svg+xml",
    ico: "image/x-icon",
    bmp: "image/bmp",
  };
  return mimeTypes[ext] || "application/octet-stream";
}

// Get language for syntax highlighting hints
function getLanguage(path: string): string {
  const ext = path.split(".").pop()?.toLowerCase() || "";
  const languages: Record<string, string> = {
    js: "javascript",
    ts: "typescript",
    jsx: "javascript",
    tsx: "typescript",
    rs: "rust",
    py: "python",
    go: "go",
    java: "java",
    c: "c",
    cpp: "cpp",
    h: "c",
    hpp: "cpp",
    css: "css",
    scss: "scss",
    json: "json",
    yaml: "yaml",
    yml: "yaml",
    toml: "toml",
    xml: "xml",
    html: "html",
    sh: "bash",
    bash: "bash",
    zsh: "bash",
    sql: "sql",
    md: "markdown",
  };
  return languages[ext] || "plaintext";
}

const FileViewer: Component = () => {
  const params = useParams<{ id: string; path: string }>();

  // Extract path from wildcard - solidjs router gives us params.path
  const filePath = createMemo(() => {
    // The path param captures everything after /view/
    const path = params.path || "";
    return path.startsWith("/") ? path : "/" + path;
  });

  const fileName = createMemo(() => {
    const path = filePath();
    return path.split("/").pop() || "Unknown";
  });

  const fileType = createMemo(() => getFileType(filePath()));

  const [bucket] = createResource(() => params.id, (id) => getBucket(id));

  const [fileContent] = createResource(
    () => ({ bucketId: params.id, path: filePath() }),
    ({ bucketId, path }) => getFile(bucketId, path)
  );

  // Convert file content based on type
  const textContent = createMemo(() => {
    const content = fileContent();
    if (!content) return "";
    return bytesToString(content);
  });

  const imageUrl = createMemo(() => {
    const content = fileContent();
    if (!content) return "";
    return bytesToDataUrl(content, getImageMimeType(filePath()));
  });

  return (
    <div class="p-6">
      {/* Header */}
      <div class="flex items-center justify-between mb-6">
        <div>
          <A
            href={`/buckets/${params.id}`}
            class="text-primary-600 dark:text-primary-400 hover:underline text-sm"
          >
            &larr; Back to Explorer
          </A>
          <h1 class="text-2xl font-bold text-gray-800 dark:text-gray-200 mt-2">
            {fileName()}
          </h1>
          <p class="text-sm text-gray-500 dark:text-gray-400 mt-1">
            <Show when={bucket()} fallback="Loading...">
              {bucket()!.name}
            </Show>
            {" - "}
            {filePath()}
          </p>
        </div>
        <div class="flex gap-2">
          <span class="px-3 py-1 text-xs font-medium rounded-full bg-gray-200 dark:bg-gray-700 text-gray-700 dark:text-gray-300">
            {getLanguage(filePath())}
          </span>
        </div>
      </div>

      {/* Content */}
      <div class="bg-white dark:bg-gray-800 rounded-lg shadow overflow-hidden">
        <Show when={fileContent.loading}>
          <div class="flex justify-center py-12">
            <div class="animate-spin rounded-full h-8 w-8 border-b-2 border-primary-600"></div>
          </div>
        </Show>

        <Show when={fileContent.error}>
          <div class="p-4 text-red-600 dark:text-red-400">
            Error loading file: {fileContent.error?.toString()}
          </div>
        </Show>

        <Show when={fileContent()}>
          <Switch fallback={<BinaryViewer bytes={fileContent()!} />}>
            <Match when={fileType() === "image"}>
              <ImageViewer url={imageUrl()} fileName={fileName()} />
            </Match>
            <Match when={fileType() === "markdown"}>
              <MarkdownViewer content={textContent()} />
            </Match>
            <Match when={fileType() === "code"}>
              <CodeViewer content={textContent()} language={getLanguage(filePath())} />
            </Match>
            <Match when={fileType() === "text"}>
              <TextViewer content={textContent()} />
            </Match>
          </Switch>
        </Show>
      </div>
    </div>
  );
};

// Sub-components for different file types

const ImageViewer: Component<{ url: string; fileName: string }> = (props) => {
  return (
    <div class="p-4 flex justify-center items-center min-h-[300px] bg-gray-50 dark:bg-gray-900">
      <img
        src={props.url}
        alt={props.fileName}
        class="max-w-full max-h-[70vh] object-contain"
      />
    </div>
  );
};

const MarkdownViewer: Component<{ content: string }> = (props) => {
  // Simple markdown rendering - in production, use a library like marked
  return (
    <div class="p-6">
      <div class="prose dark:prose-invert max-w-none">
        <pre class="whitespace-pre-wrap font-sans text-gray-800 dark:text-gray-200">
          {props.content}
        </pre>
      </div>
    </div>
  );
};

const CodeViewer: Component<{ content: string; language: string }> = (props) => {
  return (
    <div class="overflow-x-auto">
      <pre class="p-4 text-sm font-mono text-gray-800 dark:text-gray-200 bg-gray-50 dark:bg-gray-900">
        <code>{props.content}</code>
      </pre>
    </div>
  );
};

const TextViewer: Component<{ content: string }> = (props) => {
  return (
    <div class="p-4">
      <pre class="whitespace-pre-wrap font-mono text-sm text-gray-800 dark:text-gray-200">
        {props.content}
      </pre>
    </div>
  );
};

const BinaryViewer: Component<{ bytes: number[] }> = (props) => {
  return (
    <div class="p-6 text-center text-gray-500 dark:text-gray-400">
      <svg
        class="w-16 h-16 mx-auto mb-4 text-gray-400"
        fill="none"
        stroke="currentColor"
        viewBox="0 0 24 24"
      >
        <path
          stroke-linecap="round"
          stroke-linejoin="round"
          stroke-width="2"
          d="M9 12h6m-6 4h6m2 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z"
        />
      </svg>
      <p class="text-lg font-medium">Binary file</p>
      <p class="text-sm mt-1">{props.bytes.length} bytes</p>
      <p class="text-xs mt-4">This file cannot be displayed as text</p>
    </div>
  );
};

export default FileViewer;
