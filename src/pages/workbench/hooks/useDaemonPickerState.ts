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
}

export function useDaemonPickerState(props: UseDaemonPickerStateProps) {
  const { t } = useTranslation();
  const tRef = useRef(t);
  tRef.current = t;
  const [activeTab, setActiveTab] = useState<"managed" | "browse">(props.mode === "promote" ? "browse" : "managed");
  const [slugInput, setSlugInput] = useState(toManagedSlug(props.initialSlug || "cowok-project"));
  const [browseData, setBrowseData] = useState<FsBrowseResponse | null>(null);
  const [selectedPath, setSelectedPath] = useState("");
  const [isLoading, setIsLoading] = useState(false);
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [errorMsg, setErrorMsg] = useState<string | null>(null);
  const loadedForOpenRef = useRef(false);

  const loadDirectory = useCallback(async (targetPath?: string) => {
    setIsLoading(true);
    setErrorMsg(null);
    try {
      const res = await apiHarnessDaemon.browseFs(targetPath);
      setBrowseData(res);
      setSelectedPath(res.current_path);
    } catch (err: unknown) {
      setErrorMsg(extractDaemonErrorMessage(err, tRef.current("workbench.daemonActionError", "Lỗi đồng bộ Daemon SQLite V7")));
    } finally {
      setIsLoading(false);
    }
  }, []);

  useEffect(() => {
    if (!props.isOpen) {
      loadedForOpenRef.current = false;
      return;
    }
    setErrorMsg(null);
    setSlugInput(toManagedSlug(props.initialSlug || "cowok-project"));
    setActiveTab(props.mode === "promote" ? "browse" : "managed");
    if (!loadedForOpenRef.current) {
      loadedForOpenRef.current = true;
      void loadDirectory(undefined);
    }
  }, [props.isOpen, props.initialSlug, props.mode, loadDirectory]);

  return {
    activeTab, setActiveTab, slugInput, setSlugInput, browseData,
    selectedPath, setSelectedPath, isLoading, isSubmitting, setIsSubmitting,
    errorMsg, setErrorMsg, loadDirectory,
  };
}
