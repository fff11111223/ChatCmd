import { createContext, useContext } from 'react';

export type ToolCallActionHandler = (json: string) => void;

export const ToolCallActionContext = createContext<ToolCallActionHandler | undefined>(undefined);

export const useToolCallAction = () => useContext(ToolCallActionContext);
