import { getCurrentWindow } from '@tauri-apps/api/window'
import { X } from 'lucide-react'
import ReactMarkdown from 'react-markdown'
import remarkGfm from 'remark-gfm'
import rehypeSlug from 'rehype-slug'
// @ts-ignore — Vite ?raw import
import helpContent from '../../HELP.md?raw'

export default function Help() {
  const closeWindow = () => getCurrentWindow().hide()

  return (
    <div className="flex flex-col h-screen bg-white">
      {/* Header */}
      <div className="flex items-center justify-between px-5 py-4 border-b border-gray-200 flex-shrink-0">
        <span className="font-semibold text-gray-800">Help</span>
        <button onClick={closeWindow} className="text-gray-400 hover:text-gray-600">
          <X size={18} />
        </button>
      </div>

      {/* Markdown content */}
      <div className="flex-1 overflow-y-auto px-6 py-5 prose prose-sm max-w-none
        prose-headings:text-gray-800 prose-headings:font-semibold
        prose-h1:text-xl prose-h2:text-base prose-h2:mt-6 prose-h2:mb-2
        prose-h3:text-sm prose-h3:mt-4 prose-h3:mb-1
        prose-p:text-gray-600 prose-p:leading-relaxed
        prose-a:text-immich-primary prose-a:no-underline hover:prose-a:underline
        prose-code:text-immich-primary prose-code:bg-gray-100 prose-code:px-1 prose-code:rounded prose-code:text-xs
        prose-pre:bg-gray-100 prose-pre:text-xs
        prose-table:text-sm prose-td:py-1.5 prose-th:py-1.5
        prose-hr:border-gray-200 prose-li:text-gray-600">
        <ReactMarkdown remarkPlugins={[remarkGfm]} rehypePlugins={[rehypeSlug]}>
          {helpContent}
        </ReactMarkdown>
      </div>
    </div>
  )
}
