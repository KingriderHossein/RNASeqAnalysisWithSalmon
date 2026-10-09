#!/usr/bin/env Rscript
suppressPackageStartupMessages(library(DESeq2))
root <- "/home/kingrider/GSE89223_download"
out <- file.path(root,"audits/G9_validation_20261009")
dir.create(out,recursive=TRUE,showWarnings=FALSE)
stopifnot(!file.exists(file.path(out,"summary.tsv")))
z <- file(file.path(out,"run.log"),"wt")
sink(z,split=TRUE);sink(z,type="message")
on.exit({sink(type="message");sink();close(z)},add=TRUE)
cat("G9_START",format(Sys.time(),"%Y-%m-%dT%H:%M:%S%z"),"\n")
g6 <- file.path(root,"salmon/G6_SF_raw_v1")
metadata <- read.csv(file.path(root,"audits/G7_tximport_20261009/result_v1/metadata_all32_v1.csv"))
metrics <- do.call(rbind,lapply(metadata$run,function(acc) {
 d<-readLines(file.path(g6,acc,"aux_info/meta_info.json"),warn=FALSE)
 number<-function(k)as.numeric(sub(paste0('.*"',k,'"\\s*:\\s*([0-9.]+).*'),"\\1",d[grepl(paste0('"',k,'"\\s*:'),d)][1]))
 data.frame(run=acc,mapping_rate=number("percent_mapped"),num_mapped=number("num_mapped"),num_decoy=number("num_decoy_fragments"),num_processed=number("num_processed"))
}))
# restore correct sample metadata join by accession
stopifnot(identical(metadata$run,metrics$run))
outlines<-list()
for(tr in c("A","B")){
 p<-file.path(root,"audits/G8_DESeq2_20261009",paste0("track_",tr))
 dds<-readRDS(file.path(p,"dds.rds"))
 res<-read.csv(file.path(p,"deseq2_full_results.csv"))
 pcs<-read.csv(file.path(p,"pca_coordinates.csv"))
 d<-as.data.frame(colData(dds))
 nf<-normalizationFactors(dds)
 stopifnot(!is.null(nf),all(is.finite(nf)),all(nf>0))
 cooks<-assay(dds,"cooks")
 flag<-colSums(cooks>qf(.99,1,ncol(dds)-ncol(model.matrix(design(dds),d))),na.rm=TRUE)
 q<-merge(merge(pcs,metrics,by="run"),data.frame(run=colnames(dds),cooks_99pct=flag),by="run")
 stopifnot(nrow(q)==ncol(dds))
 q$distance_from_pca_center<-sqrt((q$PC1-median(q$PC1))^2+(q$PC2-median(q$PC2))^2)
 q$pca_distance_rank<-rank(-q$distance_from_pca_center,ties.method="min")
 q$low_mapping_flag<-q$mapping_rate<10
 write.csv(q,file.path(out,paste0("track_",tr,"_sample_qc.csv")),row.names=FALSE)
 cor_pca<-suppressWarnings(cor(q$PC1,q$mapping_rate,method="spearman"))
 cor_pca2<-suppressWarnings(cor(q$PC2,q$mapping_rate,method="spearman"))
 sig<-res$gene_id[!is.na(res$padj)&res$padj<.05]
 outlines[[tr]]<-list(sig=sig,counts=c(nonzero=nrow(dds),significant=length(sig),cooks_pvalue_na=sum(is.na(res$pvalue)),independent_filter_padj_na=sum(is.na(res$padj)&!is.na(res$pvalue)),sample_low_map=sum(q$low_mapping_flag),dispersions_finite=sum(is.finite(dispersions(dds))),disp_outliers=sum(mcols(dds)$dispOutlier,na.rm=TRUE)),spearman_PC1_map=cor_pca,spearman_PC2_map=cor_pca2)
 cat("TRACK",tr,"samples",nrow(q),"sig",length(sig),"PC1_mapping_rho",round(cor_pca,3),"PC2_mapping_rho",round(cor_pca2,3),"pca_top",paste(head(q$run[order(-q$distance_from_pca_center)],4),collapse=","),"lowMap",sum(q$low_mapping_flag),"CookNA",sum(is.na(res$pvalue)),"\n")
}
common<-intersect(outlines$A$sig,outlines$B$sig)
ra<-read.csv(file.path(root,"audits/G8_DESeq2_20261009/track_A/deseq2_full_results.csv"))
rb<-read.csv(file.path(root,"audits/G8_DESeq2_20261009/track_B/deseq2_full_results.csv"))
a<-ra[match(common,ra$gene_id),]
b<-rb[match(common,rb$gene_id),]
same<-sum(sign(a$log2FoldChange)==sign(b$log2FoldChange),na.rm=TRUE)
cat("TRACK_CONCORDANCE intersection",length(common),"same_sign",same,"opposite",length(common)-same,"\n")
write.csv(data.frame(gene_id=common,log2FC_A=a$log2FoldChange,log2FC_B=b$log2FoldChange,same_direction=sign(a$log2FoldChange)==sign(b$log2FoldChange)),file.path(out,"track_A_B_significant_overlap.csv"),row.names=FALSE)
summary<-data.frame(track=c("A","B"),nonzero=c(outlines$A$counts["nonzero"],outlines$B$counts["nonzero"]),sig=c(length(outlines$A$sig),length(outlines$B$sig)),mapping_low_in_track=c(outlines$A$counts["sample_low_map"],outlines$B$counts["sample_low_map"]),spearman_PC1_mapping=c(outlines$A$spearman_PC1_map,outlines$B$spearman_PC1_map),disp_outlier_genes=c(outlines$A$counts["disp_outliers"],outlines$B$counts["disp_outliers"]),cooks_na=c(outlines$A$counts["cooks_pvalue_na"],outlines$B$counts["cooks_pvalue_na"]),independent_filter_padj_na=c(outlines$A$counts["independent_filter_padj_na"],outlines$B$counts["independent_filter_padj_na"]))
write.table(summary,file.path(out,"summary.tsv"),sep="\t",quote=FALSE,row.names=FALSE)
cat("G9_AUDIT_COMPLETE",format(Sys.time(),"%Y-%m-%dT%H:%M:%S%z"),"\n")