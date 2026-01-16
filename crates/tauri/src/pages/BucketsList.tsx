import { A } from "@solidjs/router";
import { Component, createResource, createSignal, For, Show } from "solid-js";
import { listBuckets, createBucket, BucketInfo } from "../hooks/useTauri";

const BucketsList: Component = () => {
  const [buckets, { refetch }] = createResource(listBuckets);
  const [showCreateModal, setShowCreateModal] = createSignal(false);
  const [newBucketName, setNewBucketName] = createSignal("");
  const [creating, setCreating] = createSignal(false);

  const handleCreate = async () => {
    const name = newBucketName().trim();
    if (!name) return;

    setCreating(true);
    try {
      await createBucket(name);
      setNewBucketName("");
      setShowCreateModal(false);
      refetch();
    } catch (err) {
      console.error("Failed to create bucket:", err);
      alert(`Failed to create bucket: ${err}`);
    } finally {
      setCreating(false);
    }
  };

  return (
    <div class="p-6">
      <div class="flex justify-between items-center mb-6">
        <h1 class="text-2xl font-bold text-gray-800 dark:text-gray-200">
          Buckets
        </h1>
        <button
          onClick={() => setShowCreateModal(true)}
          class="px-4 py-2 bg-primary-600 text-white rounded-lg hover:bg-primary-700 transition-colors"
        >
          New Bucket
        </button>
      </div>

      <Show when={buckets.loading}>
        <div class="flex justify-center py-12">
          <div class="animate-spin rounded-full h-8 w-8 border-b-2 border-primary-600"></div>
        </div>
      </Show>

      <Show when={buckets.error}>
        <div class="bg-red-50 dark:bg-red-900 text-red-600 dark:text-red-200 p-4 rounded-lg">
          Error loading buckets: {buckets.error?.toString()}
        </div>
      </Show>

      <Show when={buckets()}>
        <Show when={buckets()!.length === 0}>
          <div class="text-center py-12 text-gray-500 dark:text-gray-400">
            <p class="text-lg mb-4">No buckets yet</p>
            <p class="text-sm">Create your first bucket to get started</p>
          </div>
        </Show>

        <div class="grid gap-4 md:grid-cols-2 lg:grid-cols-3">
          <For each={buckets()}>
            {(bucket: BucketInfo) => (
              <A
                href={`/buckets/${bucket.id}`}
                class="block p-6 bg-white dark:bg-gray-800 rounded-lg shadow hover:shadow-md transition-shadow"
              >
                <div class="flex items-center mb-2">
                  <svg
                    class="w-8 h-8 text-primary-500 mr-3"
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
                  <h3 class="text-lg font-semibold text-gray-800 dark:text-gray-200">
                    {bucket.name}
                  </h3>
                </div>
                <p class="text-sm text-gray-500 dark:text-gray-400">
                  Height: {bucket.height}
                </p>
              </A>
            )}
          </For>
        </div>
      </Show>

      {/* Create Modal */}
      <Show when={showCreateModal()}>
        <div class="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50">
          <div class="bg-white dark:bg-gray-800 rounded-lg p-6 w-full max-w-md">
            <h2 class="text-xl font-bold mb-4 text-gray-800 dark:text-gray-200">
              Create New Bucket
            </h2>
            <input
              type="text"
              value={newBucketName()}
              onInput={(e) => setNewBucketName(e.currentTarget.value)}
              placeholder="Bucket name"
              class="w-full px-4 py-2 border border-gray-300 dark:border-gray-600 rounded-lg mb-4 bg-white dark:bg-gray-700 text-gray-800 dark:text-gray-200"
              onKeyPress={(e) => e.key === "Enter" && handleCreate()}
            />
            <div class="flex justify-end gap-2">
              <button
                onClick={() => setShowCreateModal(false)}
                class="px-4 py-2 text-gray-600 dark:text-gray-400 hover:text-gray-800 dark:hover:text-gray-200"
              >
                Cancel
              </button>
              <button
                onClick={handleCreate}
                disabled={creating() || !newBucketName().trim()}
                class="px-4 py-2 bg-primary-600 text-white rounded-lg hover:bg-primary-700 disabled:opacity-50 disabled:cursor-not-allowed"
              >
                {creating() ? "Creating..." : "Create"}
              </button>
            </div>
          </div>
        </div>
      </Show>
    </div>
  );
};

export default BucketsList;
