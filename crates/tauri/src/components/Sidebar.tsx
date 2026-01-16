import { A } from "@solidjs/router";
import { Component } from "solid-js";

const Sidebar: Component = () => {
  return (
    <aside class="w-64 bg-white dark:bg-gray-800 border-r border-gray-200 dark:border-gray-700 flex flex-col">
      <div class="p-4 border-b border-gray-200 dark:border-gray-700">
        <h1 class="text-xl font-bold text-primary-600 dark:text-primary-400">
          JAX Bucket
        </h1>
        <p class="text-sm text-gray-500 dark:text-gray-400">
          Encrypted Storage
        </p>
      </div>

      <nav class="flex-1 p-4">
        <ul class="space-y-2">
          <li>
            <A
              href="/"
              class="flex items-center px-4 py-2 text-gray-700 dark:text-gray-200 rounded-lg hover:bg-gray-100 dark:hover:bg-gray-700"
              activeClass="bg-primary-50 dark:bg-primary-900 text-primary-600 dark:text-primary-400"
            >
              <svg
                class="w-5 h-5 mr-3"
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
              Buckets
            </A>
          </li>
        </ul>
      </nav>

      <div class="p-4 border-t border-gray-200 dark:border-gray-700">
        <div class="flex items-center text-sm text-gray-500 dark:text-gray-400">
          <div class="w-2 h-2 bg-green-500 rounded-full mr-2"></div>
          Gateway: localhost:8080
        </div>
      </div>
    </aside>
  );
};

export default Sidebar;
