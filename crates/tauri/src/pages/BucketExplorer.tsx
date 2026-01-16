import { A, useParams } from "@solidjs/router";
import {
  Component,
  createResource,
  createSignal,
  For,
  Show,
} from "solid-js";
import { getBucket, listFiles, FileInfo } from "../hooks/useTauri";

const BucketExplorer: Component = () => {
  const params = useParams<{ id: string }>();
  const [currentPath, setCurrentPath] = createSignal("/");

  const [bucket] = createResource(() => params.id, (id) => getBucket(id));

  const [files] = createResource(
    () => ({ bucketId: params.id, path: currentPath() }),
    ({ bucketId, path }) => listFiles(bucketId, path)
  );

  // Breadcrumb parts
  const breadcrumbs = () => {
    const path = currentPath();
    if (path === "/") return [{ name: "Root", path: "/" }];

    const parts = path.split("/").filter((p) => p);
    let acc = "";
    return [
      { name: "Root", path: "/" },
      ...parts.map((p) => {
        acc += "/" + p;
        return { name: p, path: acc };
      }),
    ];
  };

  const navigateTo = (path: string) => {
    setCurrentPath(path);
  };

  return (
    <div class="p-6">
      {/* Header */}
      <div class="flex items-center justify-between mb-6">
        <div>
          <A
            href="/"
            class="text-primary-600 dark:text-primary-400 hover:underline text-sm"
          >
            &larr; Back to Buckets
          </A>
          <h1 class="text-2xl font-bold text-gray-800 dark:text-gray-200 mt-2">
            <Show when={bucket()} fallback="Loading...">
              {bucket()!.name}
            </Show>
          </h1>
        </div>
      </div>

      {/* Breadcrumbs */}
      <div class="flex items-center gap-2 mb-4 text-sm">
        <For each={breadcrumbs()}>
          {(crumb, index) => (
            <>
              <Show when={index() > 0}>
                <span class="text-gray-400">/</span>
              </Show>
              <button
                onClick={() => navigateTo(crumb.path)}
                class="text-primary-600 dark:text-primary-400 hover:underline"
              >
                {crumb.name}
              </button>
            </>
          )}
        </For>
      </div>

      {/* File list */}
      <div class="bg-white dark:bg-gray-800 rounded-lg shadow">
        <Show when={files.loading}>
          <div class="flex justify-center py-12">
            <div class="animate-spin rounded-full h-8 w-8 border-b-2 border-primary-600"></div>
          </div>
        </Show>

        <Show when={files.error}>
          <div class="p-4 text-red-600 dark:text-red-400">
            Error loading files: {files.error?.toString()}
          </div>
        </Show>

        <Show when={files()}>
          <Show when={files()!.length === 0}>
            <div class="p-8 text-center text-gray-500 dark:text-gray-400">
              This directory is empty
            </div>
          </Show>

          <table class="w-full">
            <thead class="bg-gray-50 dark:bg-gray-700">
              <tr>
                <th class="px-6 py-3 text-left text-xs font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wider">
                  Name
                </th>
                <th class="px-6 py-3 text-left text-xs font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wider">
                  Type
                </th>
              </tr>
            </thead>
            <tbody class="divide-y divide-gray-200 dark:divide-gray-700">
              <For each={files()}>
                {(file: FileInfo) => (
                  <tr
                    class="hover:bg-gray-50 dark:hover:bg-gray-700 cursor-pointer"
                    onClick={() => {
                      if (file.is_dir) {
                        navigateTo(file.path);
                      }
                    }}
                  >
                    <td class="px-6 py-4 whitespace-nowrap">
                      <div class="flex items-center">
                        <Show
                          when={file.is_dir}
                          fallback={
                            <svg
                              class="w-5 h-5 text-gray-400 mr-3"
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
                          }
                        >
                          <svg
                            class="w-5 h-5 text-yellow-500 mr-3"
                            fill="none"
                            stroke="currentColor"
                            viewBox="0 0 24 24"
                          >
                            <path
                              stroke-linecap="round"
                              stroke-linejoin="round"
                              stroke-width="2"
                              d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z"
                            />
                          </svg>
                        </Show>
                        <Show
                          when={file.is_dir}
                          fallback={
                            <A
                              href={`/buckets/${params.id}/view${file.path}`}
                              class="text-gray-900 dark:text-gray-100 hover:text-primary-600 dark:hover:text-primary-400"
                              onClick={(e) => e.stopPropagation()}
                            >
                              {file.name}
                            </A>
                          }
                        >
                          <span class="text-gray-900 dark:text-gray-100">
                            {file.name}
                          </span>
                        </Show>
                      </div>
                    </td>
                    <td class="px-6 py-4 whitespace-nowrap text-sm text-gray-500 dark:text-gray-400">
                      {file.is_dir ? "Directory" : "File"}
                    </td>
                  </tr>
                )}
              </For>
            </tbody>
          </table>
        </Show>
      </div>
    </div>
  );
};

export default BucketExplorer;
