// @story 04.2
// @implements AC-04
import { useCallback, useEffect, useRef, useState } from "react";
import { apiHarnessDaemon } from "../../../api/harness-daemon";
import { useTranslation } from "../../../hooks/useTranslation";
import type { FsBrowseResponse } from "../../../lib/validations/harness-daemon";
import { extractDaemonErrorMessage, toManagedSlug } from "./useProjectTrashAndBrowse";

export interface UseDaemonPickerStateProps {
  isOpen: boolean;
  mode: "create" | "promote";
  initialSlug?: string;
  initialTab?: "managed" | "browse";
  initialPath?: string;
}

function resolveInitialBrowseTarget(
  resolvedDefaultTab: "managed" | "browse",
  initialPath?: string
): { targetPath: string | undefined; fallbackToHome: boolean } {
  const trimmedInit = initialPath?.trim();
  if (resolvedDefaultTab === "browse") {
    return { targetPath: trimmedInit || "~", fallbackToHome: Boolean(trimmedInit) };
  }
  return { targetPath: undefined, fallbackToHome: false };
}

function useNativeFinderAction(args: {
  currentPath: string | undefined;
  pathInput: string;
  loadDirectory: (targetPath?: string, fallbackToHome?: boolean) => Promise<void>;
  setErrorMsg: (msg: string | null) => void;
  tRef: React.MutableRefObject<(k: string, f: string) => string>;
}) {
  const { currentPath, pathInput, loadDirectory, setErrorMsg, tRef } = args;
  const [isPickingNative, setIsPickingNative] = useState(false);

  const openNativeFinder = useCallback(async () => {
    setIsPickingNative(true);
    setErrorMsg(null);
    try {
      const initPath = currentPath || pathInput.trim() || undefined;
      const res = await apiHarnessDaemon.pickNativeFolder({
        initial_path: initPath,
        prompt: tRef.current("workbench.pickerNativeFinderPrompt", "Chọn thư mục dự án trên máy"),
      });
      if (!res.cancelled && res.path) {
        await loadDirectory(res.path, false);
      }
    } catch (err: unknown) {
      setErrorMsg(extractDaemonErrorMessage(err, tRef.current("workbench.daemonActionError", "Lỗi đồng bộ Daemon SQLite V7")));
    } finally {
      setIsPickingNative(false);
    }
  }, [currentPath, loadDirectory, pathInput, setErrorMsg, tRef]);

  return { isPickingNative, openNativeFinder };
}

function useDirectoryLoader(tRef: React.MutableRefObject<(k: string, f: string) => string>) {
  const [browseData, setBrowseData] = useState<FsBrowseResponse | null>(null);
  const [selectedPath, setSelectedPath] = useState("");
  const [pathInput, setPathInput] = useState("");
  const [newSubfolderName, setNewSubfolderName] = useState("");
  const [isLoading, setIsLoading] = useState(false);
  const [errorMsg, setErrorMsg] = useState<string | null>(null);

  const loadDirectory = useCallback(async (targetPath?: string, fallbackToHome = false) => {
    setIsLoading(true);
    setErrorMsg(null);
    try {
      const res = await apiHarnessDaemon.browseFs(targetPath);
      setBrowseData(res);
      setSelectedPath(res.current_path);
      setPathInput(res.current_path);
      setNewSubfolderName("");
    } catch (err: unknown) {
      if (fallbackToHome && targetPath && targetPath !== "~") {
        await loadDirectory("~", false);
        return;
      }
      setErrorMsg(extractDaemonErrorMessage(err, tRef.current("workbench.daemonActionError", "Lỗi đồng bộ Daemon SQLite V7")));
    } finally {
      setIsLoading(false);
    }
  }, [tRef]);

  const navigateToPathInput = useCallback(() => {
    void loadDirectory(pathInput.trim() || undefined, false);
  }, [loadDirectory, pathInput]);

  const nativeFinder = useNativeFinderAction({
    currentPath: browseData?.current_path, pathInput, loadDirectory, setErrorMsg, tRef,
  });

  return {
    browseData, selectedPath, setSelectedPath, pathInput, setPathInput,
    newSubfolderName, setNewSubfolderName, isLoading, errorMsg, setErrorMsg,
    loadDirectory, navigateToPathInput, ...nativeFinder,
  };
}

export function useDaemonPickerState(props: UseDaemonPickerStateProps) {
  const { t } = useTranslation();
  const tRef = useRef(t);
  tRef.current = t;
  const resolvedDefaultTab = props.initialTab ?? (props.mode === "promote" ? "browse" : "managed");
  const [activeTab, setActiveTab] = useState<"managed" | "browse">(resolvedDefaultTab);
  const [slugInput, setSlugInput] = useState(toManagedSlug(props.initialSlug || "cowok-project"));
  const [isSubmitting, setIsSubmitting] = useState(false);
  const loadedForOpenRef = useRef(false);
  const dir = useDirectoryLoader(tRef);

  useEffect(() => {
    if (!props.isOpen) {
      loadedForOpenRef.current = false;
      return;
    }
    dir.setErrorMsg(null);
    setSlugInput(toManagedSlug(props.initialSlug || "cowok-project"));
    setActiveTab(resolvedDefaultTab);
    dir.setNewSubfolderName("");
    if (!loadedForOpenRef.current) {
      loadedForOpenRef.current = true;
      const init = resolveInitialBrowseTarget(resolvedDefaultTab, props.initialPath);
      void dir.loadDirectory(init.targetPath, init.fallbackToHome);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [props.isOpen, props.initialSlug, props.initialPath, props.mode, resolvedDefaultTab, dir.loadDirectory]);

  return {
    activeTab, setActiveTab, slugInput, setSlugInput, isSubmitting, setIsSubmitting, ...dir,
  };
}
